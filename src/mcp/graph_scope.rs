use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::errors::{Result, TokenSaveError};
use crate::mcp::tools::ToolResult;
use crate::tokensave::TokenSave;
use crate::types::NodeKind;

pub(crate) struct GraphSelector {
    pub(crate) root: PathBuf,
    pub(crate) branch: Option<String>,
}

/// One or more roots named by a single call.
///
/// `graph_root` accepts a string (one root, the #363 behaviour) or an array
/// (federate across several, #376). The array form is only meaningful for
/// tools whose answer is a list that can be merged; a whole-graph analysis
/// describes the shape of one graph, and a union of two is not a bigger
/// answer but a meaningless one, so those reject it.
pub(crate) struct GraphSelection {
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) branch: Option<String>,
}

impl GraphSelection {
    /// True when the caller named more than one root, before collapsing.
    pub(crate) fn is_federated(&self) -> bool {
        self.roots.len() > 1
    }

    /// One selector per root, carrying the shared branch.
    pub(crate) fn selectors(&self) -> Vec<GraphSelector> {
        self.roots
            .iter()
            .map(|root| GraphSelector {
                root: root.clone(),
                branch: self.branch.clone(),
            })
            .collect()
    }
}

impl GraphSelector {
    pub(crate) fn take(arguments: &mut Value) -> Result<Option<GraphSelection>> {
        let object = arguments
            .as_object_mut()
            .ok_or_else(|| config_error("tool arguments must be a JSON object"))?;
        let root = object.remove("graph_root");
        let branch = object.remove("graph_branch");

        let branch = branch
            .map(|value| required_string(&value, "graph_branch"))
            .transpose()?;
        let Some(root) = root else {
            if branch.is_some() {
                return Err(config_error(
                    "graph_branch requires a matching graph_root; omit graph_branch to query \
                     the currently served graph (selecting a different branch of the served \
                     project is not supported)",
                ));
            }
            return Ok(None);
        };

        let roots: Vec<PathBuf> = match &root {
            Value::Array(items) => {
                if items.is_empty() {
                    return Err(config_error(
                        "graph_root array must name at least one root; omit graph_root to query \
                         the currently served graph",
                    ));
                }
                items
                    .iter()
                    .map(|item| required_string(item, "graph_root").map(PathBuf::from))
                    .collect::<Result<Vec<_>>>()?
            }
            _ => vec![PathBuf::from(required_string(&root, "graph_root")?)],
        };

        Ok(Some(GraphSelection { roots, branch }))
    }
}

/// Drops roots that are worktrees of a repository already named.
///
/// Worktrees of one repo share a `git rev-parse --git-common-dir`, and each
/// carries its own `.tokensave/`, so they register as independent projects.
/// Federating across them fills the result set with copies of the same symbol
/// at slightly different line numbers, and a per-root cap does not help
/// because each worktree *is* a root and sits under the cap — the failure
/// @bobbypierce42 predicted from a machine with a dozen worktrees of one repo
/// among 100+ tracked projects.
///
/// The first root named for a given common dir wins, so the caller's ordering
/// decides which checkout represents the repo. Returns the kept roots and the
/// paths that were collapsed away, which the response reports rather than
/// discarding silently — a caller who named a root and never sees it again
/// deserves to know why.
///
/// A root that is not a git repository, or where git is unavailable, is never
/// collapsed: without the signal there is no evidence they are the same
/// source, and guessing would drop a genuinely distinct project.
pub(crate) fn collapse_worktree_roots(roots: Vec<PathBuf>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut seen_common_dirs: HashSet<PathBuf> = HashSet::new();
    let mut kept = Vec::new();
    let mut collapsed = Vec::new();

    for root in roots {
        match git_common_dir(&root) {
            Some(common) => {
                if seen_common_dirs.insert(common) {
                    kept.push(root);
                } else {
                    collapsed.push(root);
                }
            }
            None => kept.push(root),
        }
    }
    (kept, collapsed)
}

/// The repository a checkout belongs to, as an absolute path, or `None` when
/// the path is not in a git repository or git cannot be run.
fn git_common_dir(root: &Path) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(raw.trim());
    let absolute = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    absolute.canonicalize().ok()
}

pub(crate) struct GraphIdentity {
    fingerprint: String,
}

impl GraphIdentity {
    fn new(root: &str, branch: Option<&str>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(root.as_bytes());
        hasher.update([0]);
        hasher.update(branch.unwrap_or_default().as_bytes());
        let digest = hasher.finalize();
        Self {
            fingerprint: hex::encode(&digest[..16]),
        }
    }

    pub(crate) fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub(crate) fn qualify(&self, raw_id: &str) -> String {
        format!("graph:{}:{raw_id}", self.fingerprint)
    }
}

pub(crate) struct SelectedGraph {
    pub(crate) cg: TokenSave,
    pub(crate) identity: GraphIdentity,
    pub(crate) provenance_root: String,
}

/// Tools whose selected answer is a mergeable list, and may therefore be
/// federated across several roots (#376).
///
/// Deliberately narrow. `tokensave_context` is excluded despite being named in
/// the issue: it returns formatted prose sections rather than a ranked array,
/// so "interleave by score with a per-root cap" has no meaning for it and
/// concatenating per root is a different feature. Whole-graph analyses
/// (`circular`, `hotspots`, `dsm`) are excluded because their answer is a
/// property of one graph.
pub(crate) const FEDERATABLE_TOOLS: &[&str] = &["tokensave_search", "tokensave_files"];

/// How many entries each root may contribute to a federated answer.
///
/// A cap rather than a global sort because scores are BM25-derived per
/// database and are not calibrated between them: sorting two roots' scores
/// together compares numbers that do not share a scale. Round-robin by rank
/// is the honest ordering, and the cap stops one large repository crowding
/// out the others before the interleave even starts.
const PER_ROOT_CAP: usize = 25;

/// Merges per-root results into one response.
///
/// Each root's payload is parsed as a JSON array and the arrays are
/// interleaved round-robin by rank: first result from every root, then second
/// from every root, and so on. Entries keep the qualified node IDs and
/// provenance that [`qualify_result`] already attached, so a caller can tell
/// which graph any entry came from and can replay its ID safely.
///
/// A root whose payload is not an array is passed through as its own content
/// block rather than dropped — better to hand back something the caller can
/// read than to silently lose a root's answer to a shape assumption.
pub(crate) fn merge_federated_results(
    parts: Vec<(String, ToolResult)>,
    collapsed: &[PathBuf],
) -> ToolResult {
    let mut touched_files: Vec<String> = Vec::new();
    let mut roots: Vec<String> = Vec::new();
    let mut arrays: Vec<Vec<Value>> = Vec::new();
    let mut passthrough: Vec<Value> = Vec::new();

    for (root, result) in parts {
        roots.push(root);
        touched_files.extend(result.touched_files.iter().cloned());
        let blocks = result
            .value
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut entries: Option<Vec<Value>> = None;
        for block in blocks {
            let Some(text) = block.get("text").and_then(Value::as_str) else {
                continue;
            };
            // Skip the per-root provenance banner: the merged response carries
            // one banner naming every root instead.
            if text.starts_with("tokensave_graph: root=") {
                continue;
            }
            match serde_json::from_str::<Value>(text) {
                Ok(Value::Array(items)) => entries = Some(items),
                _ => passthrough.push(block),
            }
        }
        arrays.push(entries.unwrap_or_default());
    }

    // Round-robin by rank, capped per root.
    let mut merged: Vec<Value> = Vec::new();
    for position in 0..PER_ROOT_CAP {
        let mut produced = false;
        for entries in &arrays {
            if let Some(entry) = entries.get(position) {
                merged.push(entry.clone());
                produced = true;
            }
        }
        if !produced {
            break;
        }
    }

    let mut banner = format!(
        "tokensave_graph: federated across {} root(s): {}",
        roots.len(),
        roots.join(", ")
    );
    if !collapsed.is_empty() {
        let names: Vec<String> = collapsed.iter().map(|p| p.display().to_string()).collect();
        // Reported rather than dropped silently: a caller who named a root and
        // never sees it again is owed the reason.
        let _ = write!(
            banner,
            "; collapsed {} worktree(s) sharing a repository with a root above: {}",
            names.len(),
            names.join(", ")
        );
    }

    let mut content = vec![json!({"type": "text", "text": banner})];
    content.push(json!({
        "type": "text",
        "text": serde_json::to_string_pretty(&Value::Array(merged))
            .unwrap_or_else(|_| "[]".to_string())
    }));
    content.extend(passthrough);

    touched_files.sort();
    touched_files.dedup();
    ToolResult {
        value: json!({ "content": content }),
        touched_files,
    }
}

pub(crate) async fn select_graph(
    selector: GraphSelector,
    served_root: &Path,
) -> Result<SelectedGraph> {
    if !selector.root.is_absolute() {
        return Err(config_error("graph_root must be an absolute path"));
    }

    let canonical_root = selector.root.canonicalize().map_err(|error| {
        config_error(format!(
            "graph_root '{}' could not be canonicalized: {error}",
            selector.root.display()
        ))
    })?;
    if !canonical_root.is_dir() {
        return Err(config_error(format!(
            "graph_root '{}' must be a directory",
            canonical_root.display()
        )));
    }

    let canonical_served_root = served_root.canonicalize().map_err(|error| {
        config_error(format!(
            "served graph root '{}' could not be canonicalized: {error}",
            served_root.display()
        ))
    })?;
    if canonical_root == canonical_served_root {
        let remedy = if selector.branch.is_some() {
            "; omit graph_root and graph_branch to query the currently served graph \
             (selecting a different branch of the served project is not supported)"
        } else {
            "; omit graph_root to query it"
        };
        return Err(config_error(format!(
            "graph_root selects the same project already served by this MCP server{remedy}"
        )));
    }

    let canonical_utf8 = canonical_root.to_str().ok_or_else(|| {
        config_error(format!(
            "canonical graph_root '{}' is not valid UTF-8",
            canonical_root.display()
        ))
    })?;
    let provenance_root = normalize_provenance_path(canonical_utf8);
    let cg = TokenSave::open_read_only(&canonical_root, selector.branch.as_deref()).await?;
    let identity = GraphIdentity::new(&provenance_root, cg.serving_branch());

    Ok(SelectedGraph {
        cg,
        identity,
        provenance_root,
    })
}

pub(crate) fn decode_selected_inputs(
    selected: &SelectedGraph,
    arguments: &mut Value,
) -> Result<()> {
    visit_input_strings_mut(arguments, None, &mut |value, field| {
        if is_exact_raw_node_id(value) {
            return Err(config_error(format!(
                "raw node ID '{value}' must be graph-qualified for selected graph calls"
            )));
        }
        if let Some((fingerprint, raw_id)) = parse_graph_node_id(value) {
            if fingerprint != selected.identity.fingerprint() {
                return Err(config_error(
                    "graph-qualified node ID does not match graph_root or graph_branch",
                ));
            }
            *value = raw_id.to_string();
            return Ok(());
        }
        if let Some(field) = field.filter(|field| is_node_id_field(field)) {
            if value.starts_with("graph:") {
                return Err(config_error(format!(
                    "malformed graph-qualified node ID '{value}' in node ID field '{field}'"
                )));
            }
            return Err(config_error(format!(
                "malformed node ID '{value}' in node ID field '{field}'; selected graph calls require a graph-qualified node ID"
            )));
        }
        Ok(())
    })
}

pub(crate) fn validate_local_inputs(arguments: &Value) -> Result<()> {
    visit_input_strings(arguments, None, &mut |value, field| {
        if parse_graph_node_id(value).is_some() {
            return Err(config_error(
                "graph-qualified node ID cannot be used for a local call; repeat matching graph_root and graph_branch",
            ));
        }
        if let Some(field) = field.filter(|field| is_node_id_field(field)) {
            if value.starts_with("graph:") {
                return Err(config_error(format!(
                    "malformed graph-qualified node ID '{value}' in node ID field '{field}'"
                )));
            }
        }
        Ok(())
    })
}

pub(crate) async fn qualify_result(
    selected: &SelectedGraph,
    result: &mut ToolResult,
) -> Result<()> {
    let mut value = result.value.clone();
    let mut candidates = HashSet::new();
    collect_result_reference_ids(&value, &mut candidates)?;

    let candidate_ids: Vec<String> = candidates.into_iter().collect();
    let confirmed: HashSet<String> = if candidate_ids.is_empty() {
        HashSet::new()
    } else {
        selected
            .cg
            .db()
            .get_nodes_by_ids(&candidate_ids)
            .await?
            .into_iter()
            .map(|node| node.id)
            .collect()
    };

    rewrite_result_reference_ids(&mut value, &confirmed, &selected.identity)?;
    attach_provenance(selected, &mut value)?;
    result.value = value;
    Ok(())
}

fn collect_result_reference_ids(value: &Value, candidates: &mut HashSet<String>) -> Result<()> {
    collect_reference_fields(value, candidates);
    let Some(content) = value.get("content").and_then(Value::as_array) else {
        return Ok(());
    };
    for item in content {
        let Some(text) = item.get("text").and_then(Value::as_str) else {
            continue;
        };
        // Truncation can cut a raw node ID in half, so qualification cannot be
        // proven complete and returning a mix of qualified and raw IDs would let
        // a caller replay an unqualified ID against the wrong graph. Payloads
        // that carry no node IDs at all (file listings, for example) have
        // nothing to qualify and stay usable.
        if has_truncation_notice(text) && contains_raw_node_id(text) {
            return Err(config_error(
                "selected graph tool output was truncated before node references could be safely \
                 qualified; lower limit, narrow scope, or use a smaller line range and retry",
            ));
        }
        match serde_json::from_str::<Value>(text) {
            Ok(payload) => {
                collect_reference_fields(&payload, candidates);
                collect_structured_read_body(&payload, candidates)?;
            }
            Err(_) => collect_context_seen_node_ids(text, candidates)?,
        }
    }
    Ok(())
}

fn has_truncation_notice(text: &str) -> bool {
    let Some((_, notice)) = text.rsplit_once("\n\n") else {
        return false;
    };
    notice
        .strip_prefix("[... truncated at ")
        .and_then(|value| value.strip_suffix(" chars]"))
        .is_some_and(|count| !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit()))
}

fn contains_raw_node_id(text: &str) -> bool {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || character == '_' || character == ':')
    })
    .any(|token| {
        let segments: Vec<&str> = token.split(':').collect();
        segments
            .windows(2)
            .any(|pair| is_exact_raw_node_id_parts(pair[0], pair[1]))
    })
}

fn collect_reference_fields(value: &Value, candidates: &mut HashSet<String>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_reference_fields(value, candidates);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if is_reference_key(key) {
                    collect_reference_values(value, candidates);
                } else {
                    collect_reference_fields(value, candidates);
                }
            }
        }
        _ => {}
    }
}

fn collect_reference_values(value: &Value, candidates: &mut HashSet<String>) {
    match value {
        Value::String(value) if is_exact_raw_node_id(value) => {
            candidates.insert(value.clone());
        }
        Value::Array(values) => {
            for value in values {
                collect_reference_values(value, candidates);
            }
        }
        _ => {}
    }
}

fn collect_structured_read_body(payload: &Value, candidates: &mut HashSet<String>) -> Result<()> {
    let Some(object) = payload.as_object() else {
        return Ok(());
    };
    let Some(mode) = object.get("mode").and_then(Value::as_str) else {
        return Ok(());
    };
    if !matches!(mode, "map" | "signatures") {
        return Ok(());
    }
    let Some(body) = object.get("body").and_then(Value::as_str) else {
        return Ok(());
    };
    let body: Value = serde_json::from_str(body)?;
    collect_reference_fields(&body, candidates);
    Ok(())
}

fn collect_context_seen_node_ids(text: &str, candidates: &mut HashSet<String>) -> Result<()> {
    for line in text.lines() {
        let Some(ids) = line.strip_prefix("seen_node_ids: ") else {
            continue;
        };
        let ids: Value = serde_json::from_str(ids)?;
        collect_reference_values(&ids, candidates);
    }
    Ok(())
}

fn rewrite_result_reference_ids(
    value: &mut Value,
    confirmed: &HashSet<String>,
    identity: &GraphIdentity,
) -> Result<()> {
    rewrite_reference_fields(value, confirmed, identity);
    let Some(content) = value.get_mut("content").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for item in content {
        let Some(text) = item
            .get_mut("text")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let rewritten = match serde_json::from_str::<Value>(&text) {
            Ok(mut payload) => {
                let changed = rewrite_reference_fields(&mut payload, confirmed, identity)
                    | rewrite_structured_read_body(&mut payload, confirmed, identity)?;
                changed.then(|| serde_json::to_string_pretty(&payload).unwrap_or(text.clone()))
            }
            Err(_) => rewrite_context_seen_node_ids(&text, confirmed, identity)?,
        };
        if let Some(rewritten) = rewritten {
            item["text"] = Value::String(rewritten);
        }
    }
    Ok(())
}

fn rewrite_reference_fields(
    value: &mut Value,
    confirmed: &HashSet<String>,
    identity: &GraphIdentity,
) -> bool {
    match value {
        Value::Array(values) => values.iter_mut().fold(false, |changed, value| {
            rewrite_reference_fields(value, confirmed, identity) | changed
        }),
        Value::Object(values) => values.iter_mut().fold(false, |changed, (key, value)| {
            if is_reference_key(key) {
                rewrite_reference_values(value, confirmed, identity) | changed
            } else {
                rewrite_reference_fields(value, confirmed, identity) | changed
            }
        }),
        _ => false,
    }
}

fn rewrite_reference_values(
    value: &mut Value,
    confirmed: &HashSet<String>,
    identity: &GraphIdentity,
) -> bool {
    match value {
        Value::String(raw_id) if confirmed.contains(raw_id) => {
            *raw_id = identity.qualify(raw_id);
            true
        }
        Value::Array(values) => values.iter_mut().fold(false, |changed, value| {
            rewrite_reference_values(value, confirmed, identity) | changed
        }),
        _ => false,
    }
}

fn rewrite_structured_read_body(
    payload: &mut Value,
    confirmed: &HashSet<String>,
    identity: &GraphIdentity,
) -> Result<bool> {
    let Some(object) = payload.as_object_mut() else {
        return Ok(false);
    };
    let Some(mode) = object.get("mode").and_then(Value::as_str) else {
        return Ok(false);
    };
    if !matches!(mode, "map" | "signatures") {
        return Ok(false);
    }
    let Some(body) = object.get_mut("body") else {
        return Ok(false);
    };
    let Some(body_text) = body.as_str() else {
        return Ok(false);
    };
    let mut structured: Value = serde_json::from_str(body_text)?;
    if !rewrite_reference_fields(&mut structured, confirmed, identity) {
        return Ok(false);
    }
    *body = Value::String(serde_json::to_string_pretty(&structured)?);
    Ok(true)
}

fn rewrite_context_seen_node_ids(
    text: &str,
    confirmed: &HashSet<String>,
    identity: &GraphIdentity,
) -> Result<Option<String>> {
    let mut changed = false;
    let mut output = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |body| (body, "\n"));
        let Some(ids) = body.strip_prefix("seen_node_ids: ") else {
            output.push_str(line);
            continue;
        };
        let mut ids: Value = serde_json::from_str(ids)?;
        changed |= rewrite_reference_values(&mut ids, confirmed, identity);
        output.push_str("seen_node_ids: ");
        output.push_str(&serde_json::to_string(&ids)?);
        output.push_str(newline);
    }
    Ok(changed.then_some(output))
}

fn is_reference_key(key: &str) -> bool {
    key == "id" || key.ends_with("_id") || key.ends_with("_ids") || key == "dispatch_from"
}

fn required_string(value: &Value, name: &str) -> Result<String> {
    let value = value
        .as_str()
        .ok_or_else(|| config_error(format!("{name} must be a non-empty string")))?;
    if value.is_empty() {
        return Err(config_error(format!("{name} must be a non-empty string")));
    }
    Ok(value.to_string())
}

fn config_error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

fn normalize_provenance_path(path: &str) -> String {
    if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{path}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

fn parse_graph_node_id(value: &str) -> Option<(&str, &str)> {
    let rest = value.strip_prefix("graph:")?;
    let (fingerprint, raw_id) = rest.split_once(':')?;
    if fingerprint.len() != 32
        || !fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !is_exact_raw_node_id(raw_id)
    {
        return None;
    }
    Some((fingerprint, raw_id))
}

fn is_exact_raw_node_id(value: &str) -> bool {
    let Some((kind, digest)) = value.split_once(':') else {
        return false;
    };
    is_exact_raw_node_id_parts(kind, digest)
}

fn is_exact_raw_node_id_parts(kind: &str, digest: &str) -> bool {
    NodeKind::from_str(kind).is_some()
        && digest.len() == 32
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn attach_provenance(selected: &SelectedGraph, value: &mut Value) -> Result<()> {
    {
        let object = value
            .as_object()
            .ok_or_else(|| config_error("tool result must be a JSON object"))?;
        if object.get("content").is_some_and(|value| !value.is_array()) {
            return Err(config_error("tool result content must be a JSON array"));
        }
        if object.get("_meta").is_some_and(|value| !value.is_object()) {
            return Err(config_error("tool result _meta must be a JSON object"));
        }
    }

    let branch = selected.cg.serving_branch().unwrap_or("single-db");
    let encoded_root = serde_json::to_string(&selected.provenance_root)?;
    let encoded_branch = serde_json::to_string(branch)?;
    let banner = json!({
        "type": "text",
        "text": format!(
            "tokensave_graph: root={encoded_root} branch={encoded_branch} read_only=true"
        )
    });
    let provenance = json!({
        "graph_root": selected.provenance_root,
        "graph_branch": selected.cg.serving_branch(),
        "selected": true,
        "read_only": true
    });
    let Some(object) = value.as_object_mut() else {
        unreachable!("tool result object shape was validated above");
    };

    match object.get_mut("content") {
        Some(Value::Array(content)) => content.insert(0, banner),
        None => {
            object.insert("content".to_string(), Value::Array(vec![banner]));
        }
        Some(_) => unreachable!("tool result content shape was validated above"),
    }
    match object.get_mut("_meta") {
        Some(Value::Object(meta)) => {
            meta.insert("tokensave".to_string(), provenance);
        }
        None => {
            object.insert("_meta".to_string(), json!({ "tokensave": provenance }));
        }
        Some(_) => unreachable!("tool result metadata shape was validated above"),
    }
    Ok(())
}

fn is_node_id_field(field: &str) -> bool {
    field == "id" || field.ends_with("_id") || field.ends_with("_ids") || field == "dispatch_from"
}

fn visit_input_strings(
    value: &Value,
    field: Option<&str>,
    visitor: &mut impl FnMut(&str, Option<&str>) -> Result<()>,
) -> Result<()> {
    match value {
        Value::String(value) => visitor(value, field),
        Value::Array(values) => {
            for value in values {
                visit_input_strings(value, field, visitor)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (field, value) in values {
                visit_input_strings(value, Some(field), visitor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn visit_input_strings_mut(
    value: &mut Value,
    field: Option<&str>,
    visitor: &mut impl FnMut(&mut String, Option<&str>) -> Result<()>,
) -> Result<()> {
    match value {
        Value::String(value) => visitor(value, field),
        Value::Array(values) => {
            for value in values {
                visit_input_strings_mut(value, field, visitor)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (field, value) in values {
                visit_input_strings_mut(value, Some(field), visitor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::path::Path;

    use serde_json::{json, Value};
    use tempfile::TempDir;

    use super::*;
    use crate::db::Database;
    use crate::mcp::tools::ToolResult;
    use crate::tokensave::TokenSave;
    use crate::types::{Node, NodeKind, Visibility};

    const RAW_ID: &str = "function:0123456789abcdef0123456789abcdef";
    const MISSING_ID: &str = "function:ffffffffffffffffffffffffffffffff";

    fn error_text<T>(result: crate::errors::Result<T>) -> String {
        match result {
            Ok(_) => panic!("expected error"),
            Err(error) => error.to_string(),
        }
    }

    fn sample_node() -> Node {
        Node {
            id: RAW_ID.to_string(),
            kind: NodeKind::Function,
            name: "sample".to_string(),
            qualified_name: "sample::sample".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 1,
            attrs_start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 1,
            signature: None,
            docstring: None,
            visibility: Visibility::Private,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: 0,
            parent_id: None,
        }
    }

    async fn initialized_graph(with_node: bool) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let graph = TokenSave::init(dir.path()).await.unwrap();
        drop(graph);

        if with_node {
            let db_path = dir.path().join(".tokensave/tokensave.db");
            let (db, _) = Database::open(&db_path).await.unwrap();
            db.insert_node(&sample_node()).await.unwrap();
            db.checkpoint().await.unwrap();
        }

        dir
    }

    async fn selected_graph(with_node: bool) -> (TempDir, TempDir, SelectedGraph) {
        let served = tempfile::tempdir().unwrap();
        let graph = initialized_graph(with_node).await;
        let selector = GraphSelector {
            root: graph.path().to_path_buf(),
            branch: None,
        };
        let selected = select_graph(selector, served.path()).await.unwrap();
        (served, graph, selected)
    }

    #[test]
    fn selector_removes_valid_fields_from_arguments() {
        let mut arguments = json!({
            "query": "sample",
            "graph_root": "/tmp/other",
            "graph_branch": "feature"
        });

        let selector = GraphSelector::take(&mut arguments).unwrap().unwrap();

        assert_eq!(selector.roots, vec![PathBuf::from("/tmp/other")]);
        assert_eq!(selector.branch.as_deref(), Some("feature"));
        assert_eq!(arguments, json!({ "query": "sample" }));
    }

    #[test]
    fn selector_absent_returns_none() {
        let mut arguments = json!({ "query": "sample" });
        assert!(GraphSelector::take(&mut arguments).unwrap().is_none());
        assert_eq!(arguments, json!({ "query": "sample" }));
    }

    #[test]
    fn selector_rejects_invalid_values_and_branch_without_root() {
        for (arguments, needle) in [
            (json!({ "graph_root": "" }), "graph_root"),
            (json!({ "graph_root": 7 }), "graph_root"),
            (
                json!({ "graph_root": "/tmp/other", "graph_branch": "" }),
                "graph_branch",
            ),
            (
                json!({ "graph_root": "/tmp/other", "graph_branch": 7 }),
                "graph_branch",
            ),
            (json!({ "graph_branch": "feature" }), "graph_root"),
        ] {
            let mut arguments = arguments;
            let message = error_text(GraphSelector::take(&mut arguments));
            assert!(message.contains(needle), "{message}");
        }
    }

    #[tokio::test]
    async fn selection_rejects_path_errors_and_served_root() {
        let served = tempfile::tempdir().unwrap();
        let missing = served.path().join("missing");
        let file = served.path().join("file");
        std::fs::write(&file, "not a directory").unwrap();

        for (root, needle) in [
            (Path::new("relative").to_path_buf(), "absolute"),
            (missing, "canonical"),
            (file, "directory"),
            (served.path().to_path_buf(), "same"),
        ] {
            let selector = GraphSelector { root, branch: None };
            let message = error_text(select_graph(selector, served.path()).await);
            assert!(message.contains(needle), "{message}");
        }
    }

    #[tokio::test]
    async fn same_root_rejection_guides_omission_and_branch_limits() {
        let served = tempfile::tempdir().unwrap();

        let message = error_text(
            select_graph(
                GraphSelector {
                    root: served.path().to_path_buf(),
                    branch: None,
                },
                served.path(),
            )
            .await,
        );
        assert!(message.contains("same project"), "{message}");
        assert!(message.contains("omit graph_root to query it"), "{message}");

        let message = error_text(
            select_graph(
                GraphSelector {
                    root: served.path().to_path_buf(),
                    branch: Some("feature".to_string()),
                },
                served.path(),
            )
            .await,
        );
        assert!(
            message.contains("omit graph_root and graph_branch"),
            "{message}"
        );
        assert!(message.contains("not supported"), "{message}");
    }

    #[test]
    fn branch_without_root_guides_omission_and_branch_limits() {
        let mut arguments = json!({ "graph_branch": "feature" });

        let message = error_text(GraphSelector::take(&mut arguments));

        assert!(
            message.contains("requires a matching graph_root"),
            "{message}"
        );
        assert!(message.contains("omit graph_branch"), "{message}");
        assert!(message.contains("not supported"), "{message}");
    }

    #[tokio::test]
    async fn selection_uses_exact_root_without_walk_up() {
        let served = tempfile::tempdir().unwrap();
        let graph = initialized_graph(false).await;
        let child = graph.path().join("child");
        std::fs::create_dir(&child).unwrap();

        let message = error_text(
            select_graph(
                GraphSelector {
                    root: child,
                    branch: None,
                },
                served.path(),
            )
            .await,
        );

        assert!(message.contains("not an initialized TokenSave project root"));
    }

    #[tokio::test]
    async fn selection_propagates_open_and_branch_errors() {
        let served = tempfile::tempdir().unwrap();
        let uninitialized = tempfile::tempdir().unwrap();
        let message = error_text(
            select_graph(
                GraphSelector {
                    root: uninitialized.path().to_path_buf(),
                    branch: None,
                },
                served.path(),
            )
            .await,
        );
        assert!(message.contains("not an initialized TokenSave project root"));

        let graph = initialized_graph(false).await;
        let message = error_text(
            select_graph(
                GraphSelector {
                    root: graph.path().to_path_buf(),
                    branch: Some("feature".to_string()),
                },
                served.path(),
            )
            .await,
        );
        assert!(message.contains("branch tracking"));
    }

    #[test]
    fn identity_is_deterministic_and_separates_root_and_branch() {
        let one = GraphIdentity::new("/tmp/ab", Some("c"));
        let same = GraphIdentity::new("/tmp/ab", Some("c"));
        let root_collision = GraphIdentity::new("/tmp/a", Some("bc"));
        let single_db = GraphIdentity::new("/tmp/ab", None);

        assert_eq!(one.fingerprint(), same.fingerprint());
        assert_ne!(one.fingerprint(), root_collision.fingerprint());
        assert_ne!(one.fingerprint(), single_db.fingerprint());
        assert_eq!(one.fingerprint().len(), 32);
        assert!(one
            .fingerprint()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_eq!(
            one.qualify(RAW_ID),
            format!("graph:{}:{RAW_ID}", one.fingerprint())
        );
    }

    #[test]
    fn windows_provenance_normalization_is_pure_and_portable() {
        assert_eq!(
            normalize_provenance_path(r"\\?\C:\src\project"),
            r"C:\src\project"
        );
        assert_eq!(
            normalize_provenance_path(r"\\?\UNC\server\share\project"),
            r"\\server\share\project"
        );
        assert_eq!(
            normalize_provenance_path(r"C:\src\project"),
            r"C:\src\project"
        );
        assert_eq!(normalize_provenance_path("/src/project"), "/src/project");
    }

    #[tokio::test]
    async fn selected_input_decoder_handles_scalars_arrays_and_recursion() {
        let (_served, _graph, selected) = selected_graph(false).await;
        let qualified = selected.identity.qualify(RAW_ID);
        let mut arguments = json!({
            "node_id": qualified,
            "id": selected.identity.qualify(RAW_ID),
            "node_ids": [selected.identity.qualify(RAW_ID)],
            "nested": {
                "exclude_node_ids": [selected.identity.qualify(RAW_ID)]
            },
            "other": selected.identity.qualify(RAW_ID),
            "query": format!("prose containing {RAW_ID}")
        });

        decode_selected_inputs(&selected, &mut arguments).unwrap();

        assert_eq!(arguments["node_id"], RAW_ID);
        assert_eq!(arguments["id"], RAW_ID);
        assert_eq!(arguments["node_ids"], json!([RAW_ID]));
        assert_eq!(arguments["nested"]["exclude_node_ids"], json!([RAW_ID]));
        assert_eq!(arguments["other"], RAW_ID);
        assert_eq!(arguments["query"], format!("prose containing {RAW_ID}"));
    }

    #[tokio::test]
    async fn selected_decoder_leaves_free_form_graph_prefixes_unchanged() {
        let (_served, _graph, selected) = selected_graph(false).await;
        let mut arguments = json!({
            "query": "graph: algorithms",
            "nested": ["graph: notes"]
        });
        let original = arguments.clone();

        decode_selected_inputs(&selected, &mut arguments).unwrap();

        assert_eq!(arguments, original);
    }

    #[tokio::test]
    async fn selected_decoder_rejects_malformed_graph_ids_in_known_fields() {
        let (_served, _graph, selected) = selected_graph(false).await;
        for mut arguments in [
            json!({ "node_id": "graph: algorithms" }),
            json!({ "id": "graph: algorithms" }),
            json!({ "node_ids": ["graph: algorithms"] }),
            json!({ "exclude_node_ids": ["graph: algorithms"] }),
            json!({ "from_id": "graph: algorithms" }),
            json!({ "to_id": "graph: algorithms" }),
            json!({ "dispatch_from": "graph: algorithms" }),
        ] {
            let message = error_text(decode_selected_inputs(&selected, &mut arguments));
            assert!(
                message.contains("malformed graph-qualified node ID"),
                "{message}"
            );
        }
    }

    #[tokio::test]
    async fn selected_decoder_rejects_malformed_unqualified_ids_in_known_fields() {
        let (_served, _graph, selected) = selected_graph(false).await;
        for mut arguments in [
            json!({ "node_id": "function:short" }),
            json!({ "id": "unknown:0123456789abcdef0123456789abcdef" }),
            json!({ "parent_id": "function:not-hexadecimal-at-all" }),
            json!({ "node_ids": ["function:short"] }),
            json!({ "exclude_node_ids": ["function:short"] }),
            json!({ "dispatch_from": "function:short" }),
        ] {
            let message = error_text(decode_selected_inputs(&selected, &mut arguments));
            assert!(message.contains("malformed node ID"), "{message}");
        }

        let mut arguments = json!({
            "query": "function:short",
            "nested": ["unknown:short"]
        });
        let original = arguments.clone();
        decode_selected_inputs(&selected, &mut arguments).unwrap();
        assert_eq!(arguments, original);
    }

    #[tokio::test]
    async fn selected_decoder_rejects_empty_ids_but_preserves_free_form_empty_strings() {
        let (_served, _graph, selected) = selected_graph(false).await;
        for mut arguments in [
            json!({ "node_id": "" }),
            json!({ "id": "" }),
            json!({ "node_ids": [""] }),
            json!({ "exclude_node_ids": [""] }),
        ] {
            let message = error_text(decode_selected_inputs(&selected, &mut arguments));
            assert!(message.contains("malformed node ID"), "{message}");
        }

        let mut arguments = json!({
            "query": "",
            "nested": [""]
        });
        let original = arguments.clone();
        decode_selected_inputs(&selected, &mut arguments).unwrap();
        assert_eq!(arguments, original);
    }

    #[tokio::test]
    async fn selected_input_decoder_rejects_raw_malformed_and_wrong_identity_ids() {
        let (_served, _graph, selected) = selected_graph(false).await;
        let wrong = GraphIdentity::new("/tmp/wrong", None);
        for (value, needle) in [
            (RAW_ID.to_string(), "qualified"),
            (format!("graph:not-hex:{RAW_ID}"), "malformed"),
            (wrong.qualify(RAW_ID), "graph_root"),
            (
                format!(
                    "graph:{}:function:0123456789abcdef",
                    selected.identity.fingerprint()
                ),
                "malformed",
            ),
        ] {
            let mut arguments = json!({ "node_id": value });
            let message = error_text(decode_selected_inputs(&selected, &mut arguments));
            assert!(message.contains(needle), "{message}");
        }

        let mut free_form_raw = json!({ "query": RAW_ID });
        let message = error_text(decode_selected_inputs(&selected, &mut free_form_raw));
        assert!(message.contains("must be graph-qualified"), "{message}");
    }

    #[tokio::test]
    async fn branch_identity_mismatch_is_rejected() {
        let (_served, _graph, selected) = selected_graph(false).await;
        let wrong_branch = GraphIdentity::new(&selected.provenance_root, Some("other"));
        let mut arguments = json!({ "node_id": wrong_branch.qualify(RAW_ID) });

        let message = error_text(decode_selected_inputs(&selected, &mut arguments));

        assert!(message.contains("graph_branch"), "{message}");
    }

    #[test]
    fn local_validator_rejects_exact_qualified_ids_but_not_prose() {
        let qualified = GraphIdentity::new("/tmp/graph", None).qualify(RAW_ID);
        let message = error_text(validate_local_inputs(&json!({
            "node_ids": [qualified.clone()]
        })));
        assert!(message.contains("repeat matching graph_root"), "{message}");

        validate_local_inputs(&json!({
            "query": format!("prose containing {qualified}")
        }))
        .unwrap();
    }

    #[test]
    fn local_validator_leaves_free_form_graph_prefixes_unchanged() {
        validate_local_inputs(&json!({
            "query": "graph: algorithms",
            "nested": ["graph: notes"]
        }))
        .unwrap();
    }

    #[test]
    fn local_validator_rejects_malformed_graph_ids_in_known_fields() {
        for arguments in [
            json!({ "node_id": "graph: algorithms" }),
            json!({ "id": "graph: algorithms" }),
            json!({ "node_ids": ["graph: algorithms"] }),
            json!({ "exclude_node_ids": ["graph: algorithms"] }),
            json!({ "from_id": "graph: algorithms" }),
            json!({ "to_id": "graph: algorithms" }),
        ] {
            let message = error_text(validate_local_inputs(&arguments));
            assert!(
                message.contains("malformed graph-qualified node ID"),
                "{message}"
            );
        }
    }

    #[test]
    fn raw_id_recognition_requires_valid_kind_and_hex_length() {
        assert!(is_exact_raw_node_id(RAW_ID));
        assert!(!is_exact_raw_node_id(
            "unknown:0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_exact_raw_node_id(
            "function:0123456789abcdef0123456789abcde"
        ));
        assert!(!is_exact_raw_node_id(
            "function:0123456789abcdef0123456789abcdef0"
        ));
    }

    #[tokio::test]
    async fn qualify_result_is_atomic_when_provenance_shape_is_invalid() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let mut result = ToolResult {
            value: json!({
                "content": "not an array",
                "structured": { "id": RAW_ID }
            }),
            touched_files: vec![],
        };
        let original = result.value.clone();
        let original_bytes = serde_json::to_vec(&original).unwrap();

        let message = error_text(qualify_result(&selected, &mut result).await);

        assert!(
            message.contains("content must be a JSON array"),
            "{message}"
        );
        assert_eq!(result.value, original);
        assert_eq!(serde_json::to_vec(&result.value).unwrap(), original_bytes);
    }

    #[tokio::test]
    async fn qualify_result_rewrites_only_confirmed_reference_fields() {
        let (_served, graph, selected) = selected_graph(true).await;
        let qualified = selected.identity.qualify(RAW_ID);
        let mut result = ToolResult {
            value: json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&json!({
                        "id": RAW_ID,
                        "node_ids": [RAW_ID, null],
                        "parent_id": RAW_ID,
                        "dispatch_from": RAW_ID,
                        "missing_id": MISSING_ID,
                        "source": RAW_ID,
                        "signature": RAW_ID,
                        "prose": format!("mentions {RAW_ID}")
                    })).unwrap()
                }],
                "structured": {
                    "id": RAW_ID,
                    "missing_id": MISSING_ID,
                    "source": RAW_ID
                }
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        assert_eq!(result.value["structured"]["id"], qualified);
        assert_eq!(result.value["structured"]["missing_id"], MISSING_ID);
        assert_eq!(result.value["structured"]["source"], RAW_ID);
        let content = result.value["content"].as_array().unwrap();
        assert_eq!(
            content[0]["text"],
            format!(
                "tokensave_graph: root={} branch=\"single-db\" read_only=true",
                serde_json::to_string(&normalize_provenance_path(
                    graph.path().canonicalize().unwrap().to_str().unwrap()
                ))
                .unwrap()
            )
        );
        let body: Value = serde_json::from_str(content[1]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["id"], qualified);
        assert_eq!(body["node_ids"], json!([qualified, null]));
        assert_eq!(body["parent_id"], qualified);
        assert_eq!(body["dispatch_from"], qualified);
        assert_eq!(body["missing_id"], MISSING_ID);
        assert_eq!(body["source"], RAW_ID);
        assert_eq!(body["signature"], RAW_ID);
        assert_eq!(body["prose"], format!("mentions {RAW_ID}"));
        assert_eq!(
            result.value["_meta"]["tokensave"]["graph_root"],
            normalize_provenance_path(graph.path().canonicalize().unwrap().to_str().unwrap())
        );
        assert_eq!(
            result.value["_meta"]["tokensave"]["graph_branch"],
            Value::Null
        );
        assert_eq!(result.value["_meta"]["tokensave"]["selected"], true);
        assert_eq!(result.value["_meta"]["tokensave"]["read_only"], true);
    }

    #[tokio::test]
    async fn text_provenance_json_escapes_values_onto_one_line() {
        let (_served, _graph, mut selected) = selected_graph(false).await;
        selected.provenance_root = "root\nwith\rcontrols\u{0008}".to_string();
        let mut result = ToolResult {
            value: json!({ "content": [] }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        let banner = result.value["content"][0]["text"].as_str().unwrap();
        assert_eq!(banner.lines().count(), 1, "{banner:?}");
        assert_eq!(
            banner,
            r#"tokensave_graph: root="root\nwith\rcontrols\b" branch="single-db" read_only=true"#
        );
        assert_eq!(
            result.value["_meta"]["tokensave"]["graph_root"],
            "root\nwith\rcontrols\u{0008}"
        );
        assert_eq!(
            result.value["_meta"]["tokensave"]["graph_branch"],
            Value::Null
        );
    }

    #[tokio::test]
    async fn qualify_result_rewrites_context_seen_node_ids_only() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let qualified = selected.identity.qualify(RAW_ID);
        let mut result = ToolResult {
            value: json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Code containing {RAW_ID}\n\nseen_node_ids: [\"{RAW_ID}\",\"{MISSING_ID}\"]\n"
                    )
                }]
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        let text = result.value["content"][1]["text"].as_str().unwrap();
        assert!(
            text.contains(&format!("Code containing {RAW_ID}")),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "seen_node_ids: [\"{qualified}\",\"{MISSING_ID}\"]"
            )),
            "{text}"
        );
    }

    #[tokio::test]
    async fn qualify_result_rewrites_structured_read_body_but_preserves_source_modes() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let qualified = selected.identity.qualify(RAW_ID);
        let structured_body = serde_json::to_string_pretty(&json!({
            "symbols": [{
                "id": RAW_ID,
                "signature": RAW_ID,
                "source": RAW_ID
            }]
        }))
        .unwrap();
        let mut result = ToolResult {
            value: json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&json!({
                            "file": "src/lib.rs",
                            "mode": "map",
                            "body": structured_body
                        })).unwrap()
                    },
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&json!({
                            "file": "src/lib.rs",
                            "mode": "signatures",
                            "body": structured_body
                        })).unwrap()
                    },
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&json!({
                            "file": "src/lib.rs",
                            "mode": "full",
                            "body": RAW_ID
                        })).unwrap()
                    },
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&json!({
                            "file": "src/lib.rs",
                            "mode": "lines",
                            "body": RAW_ID
                        })).unwrap()
                    }
                ]
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        let content = result.value["content"].as_array().unwrap();
        for item in &content[1..=2] {
            let payload: Value = serde_json::from_str(item["text"].as_str().unwrap()).unwrap();
            let body: Value = serde_json::from_str(payload["body"].as_str().unwrap()).unwrap();
            assert_eq!(body["symbols"][0]["id"], qualified);
            assert_eq!(body["symbols"][0]["signature"], RAW_ID);
            assert_eq!(body["symbols"][0]["source"], RAW_ID);
        }
        for item in &content[3..=4] {
            let payload: Value = serde_json::from_str(item["text"].as_str().unwrap()).unwrap();
            assert_eq!(payload["body"], RAW_ID);
        }
    }

    #[tokio::test]
    async fn qualify_result_does_not_double_qualify_references() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let qualified = selected.identity.qualify(RAW_ID);
        let mut result = ToolResult {
            value: json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&json!({ "id": qualified })).unwrap()
                }]
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        let payload: Value =
            serde_json::from_str(result.value["content"][1]["text"].as_str().unwrap()).unwrap();
        assert_eq!(payload["id"], qualified);
    }

    #[tokio::test]
    async fn qualify_result_rejects_truncated_structured_text_atomically() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let text = format!(
            "{{\"id\":\"{RAW_ID}\",\"padding\":\"{}\n\n[... truncated at 15000 chars]",
            "x".repeat(15_100)
        );
        let mut result = ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": text }]
            }),
            touched_files: vec!["src/lib.rs".to_string()],
        };
        let original = result.value.clone();

        let message = error_text(qualify_result(&selected, &mut result).await);

        assert!(message.contains("truncated"), "{message}");
        assert!(message.contains("lower limit"), "{message}");
        assert!(message.contains("narrow scope"), "{message}");
        assert!(message.contains("smaller line range"), "{message}");
        assert_eq!(result.value, original);
        assert_eq!(result.touched_files, vec!["src/lib.rs"]);
    }

    #[tokio::test]
    async fn qualify_result_allows_truncated_output_without_node_ids() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let text = format!(
            "src/lib.rs\n{}\n\n[... truncated at 15000 chars]",
            "src/other.rs\n".repeat(1_200)
        );
        let mut result = ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": text }]
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        assert_eq!(result.value["content"][1]["text"], text);
    }

    #[tokio::test]
    async fn qualify_result_preserves_untruncated_non_json_prose_without_trailer() {
        let (_served, _graph, selected) = selected_graph(true).await;
        let prose = format!("ordinary prose containing {RAW_ID}");
        let mut result = ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": prose }]
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        assert_eq!(result.value["content"][1]["text"], prose);
    }
}
