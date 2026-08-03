use std::collections::HashSet;
use std::ops::Range;
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

impl GraphSelector {
    pub(crate) fn take(arguments: &mut Value) -> Result<Option<Self>> {
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
                return Err(config_error("graph_branch requires a matching graph_root"));
            }
            return Ok(None);
        };

        Ok(Some(Self {
            root: PathBuf::from(required_string(&root, "graph_root")?),
            branch,
        }))
    }
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
    pub(crate) canonical_root: PathBuf,
    pub(crate) provenance_root: String,
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
        return Err(config_error(
            "graph_root selects the same project already served by this MCP server",
        ));
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
        canonical_root,
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
    visit_strings(&value, &mut |value| {
        for range in raw_node_id_ranges(value) {
            if !is_already_qualified(value, range.start) {
                candidates.insert(value[range].to_string());
            }
        }
        Ok(())
    })?;

    let candidate_ids: Vec<String> = candidates.into_iter().collect();
    let confirmed: HashSet<String> = selected
        .cg
        .db()
        .get_nodes_by_ids(&candidate_ids)
        .await?
        .into_iter()
        .map(|node| node.id)
        .collect();

    visit_strings_mut(&mut value, &mut |value| {
        *value = qualify_confirmed_ids(value, &confirmed, &selected.identity);
        Ok(())
    })?;

    attach_provenance(selected, &mut value)?;
    result.value = value;
    Ok(())
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
    NodeKind::from_str(kind).is_some()
        && digest.len() == 32
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn raw_node_id_ranges(value: &str) -> Vec<Range<usize>> {
    let bytes = value.as_bytes();
    let mut ranges = Vec::new();
    let mut start = 0;

    while start < bytes.len() {
        if !is_token_byte(bytes[start]) {
            start += 1;
            continue;
        }
        if start > 0 && is_token_byte(bytes[start - 1]) {
            start += 1;
            continue;
        }

        let mut colon = start;
        while colon < bytes.len() && is_token_byte(bytes[colon]) {
            colon += 1;
        }
        if colon == bytes.len() || bytes[colon] != b':' {
            start = colon;
            continue;
        }

        let kind = &value[start..colon];
        let end = colon + 33;
        let digest = bytes.get(colon + 1..end);
        if NodeKind::from_str(kind).is_some()
            && digest.is_some_and(|digest| digest.iter().all(u8::is_ascii_hexdigit))
            && (end == bytes.len() || !is_token_byte(bytes[end]))
        {
            ranges.push(start..end);
            start = end;
        } else {
            start = colon + 1;
        }
    }

    ranges
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_already_qualified(value: &str, raw_start: usize) -> bool {
    let Some(prefix) = value.get(..raw_start) else {
        return false;
    };
    let Some(graph_start) = prefix.rfind("graph:") else {
        return false;
    };
    let fingerprint = &prefix[graph_start + "graph:".len()..];
    fingerprint.len() == 33
        && fingerprint.ends_with(':')
        && fingerprint[..32]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && (graph_start == 0 || !is_token_byte(prefix.as_bytes()[graph_start - 1]))
}

fn qualify_confirmed_ids(
    value: &str,
    confirmed: &HashSet<String>,
    identity: &GraphIdentity,
) -> String {
    let ranges = raw_node_id_ranges(value);
    if ranges.is_empty() {
        return value.to_string();
    }

    let mut output = String::with_capacity(value.len());
    let mut copied_until = 0;
    for range in ranges {
        output.push_str(&value[copied_until..range.start]);
        let raw_id = &value[range.clone()];
        if confirmed.contains(raw_id) && !is_already_qualified(value, range.start) {
            output.push_str(&identity.qualify(raw_id));
        } else {
            output.push_str(raw_id);
        }
        copied_until = range.end;
    }
    output.push_str(&value[copied_until..]);
    output
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
    let banner = json!({
        "type": "text",
        "text": format!(
            "tokensave_graph: root={} branch={branch} read_only=true",
            selected.provenance_root
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
    matches!(
        field,
        "id" | "node_id" | "node_ids" | "exclude_node_ids" | "from_id" | "to_id"
    )
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

fn visit_strings(value: &Value, visitor: &mut impl FnMut(&str) -> Result<()>) -> Result<()> {
    match value {
        Value::String(value) => visitor(value),
        Value::Array(values) => {
            for value in values {
                visit_strings(value, visitor)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for value in values.values() {
                visit_strings(value, visitor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn visit_strings_mut(
    value: &mut Value,
    visitor: &mut impl FnMut(&mut String) -> Result<()>,
) -> Result<()> {
    match value {
        Value::String(value) => visitor(value),
        Value::Array(values) => {
            for value in values {
                visit_strings_mut(value, visitor)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                visit_strings_mut(value, visitor)?;
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

        assert_eq!(selector.root, Path::new("/tmp/other"));
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
        ] {
            let message = error_text(decode_selected_inputs(&selected, &mut arguments));
            assert!(
                message.contains("malformed graph-qualified node ID"),
                "{message}"
            );
        }
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
    fn raw_id_recognition_requires_valid_kind_hex_length_and_boundaries() {
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

        let prose = format!("x{RAW_ID} {RAW_ID}0 ({RAW_ID})");
        let matches = raw_node_id_ranges(&prose);
        assert_eq!(matches.len(), 1);
        assert_eq!(&prose[matches[0].clone()], RAW_ID);
        assert!(raw_node_id_ranges("function:ééééééééééééééé€").is_empty());
    }

    #[test]
    fn raw_id_scanner_handles_max_size_punctuation_input() {
        let input = ".,;!".repeat(3_750);
        assert_eq!(input.len(), 15_000);
        assert!(raw_node_id_ranges(&input).is_empty());
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
    async fn qualify_result_rewrites_confirmed_ids_and_attaches_single_db_provenance() {
        let (_served, graph, selected) = selected_graph(true).await;
        let qualified = selected.identity.qualify(RAW_ID);
        let already_qualified = qualified.clone();
        let mut result = ToolResult {
            value: json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "structured={RAW_ID}; missing={MISSING_ID}; existing={already_qualified}"
                    )
                }],
                "structured": {
                    "id": RAW_ID,
                    "missing": MISSING_ID,
                    "nested": [RAW_ID]
                }
            }),
            touched_files: vec![],
        };

        qualify_result(&selected, &mut result).await.unwrap();

        assert_eq!(result.value["structured"]["id"], qualified);
        assert_eq!(result.value["structured"]["nested"], json!([qualified]));
        assert_eq!(result.value["structured"]["missing"], MISSING_ID);
        let content = result.value["content"].as_array().unwrap();
        assert_eq!(
            content[0]["text"],
            format!(
                "tokensave_graph: root={} branch=single-db read_only=true",
                normalize_provenance_path(graph.path().canonicalize().unwrap().to_str().unwrap())
            )
        );
        let body = content[1]["text"].as_str().unwrap();
        assert!(body.contains(&format!("structured={qualified}")), "{body}");
        assert!(body.contains(&format!("missing={MISSING_ID}")), "{body}");
        assert!(
            body.contains(&format!("existing={already_qualified}")),
            "{body}"
        );
        assert!(!body.contains("graph:graph:"), "{body}");
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
}
