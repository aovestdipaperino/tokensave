//! `tokensave_knowledge`: search and read imported OKF bundles (#858).

use serde_json::json;
use serde_json::Value;

use crate::errors::Result;
use crate::errors::TokenSaveError;
use crate::okf::knowledge;
use crate::okf::knowledge::Concept;
use crate::okf::knowledge::Filter;
use crate::tokensave::TokenSave;

use super::super::ToolResult;
use super::truncate_response;

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;
const MAX_BODY_CHARS: usize = 24_000;
const MAX_LISTED_LINKS: usize = 100;

fn text_result(value: &Value) -> ToolResult {
    let formatted = serde_json::to_string_pretty(value).unwrap_or_default();
    ToolResult {
        value: json!({ "content": [{ "type": "text", "text": truncate_response(&formatted) }] }),
        touched_files: vec![],
    }
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

async fn code_files(cg: &TokenSave, concept: &Concept) -> Vec<Value> {
    let mut out = Vec::with_capacity(concept.code_paths.len());
    for path in &concept.code_paths {
        out.push(json!({ "path": path, "indexed": cg.is_indexed_file(path).await }));
    }
    out
}

fn bundles_summary(knowledge: &knowledge::Knowledge) -> Vec<Value> {
    let counts = knowledge.counts();
    knowledge
        .bundles
        .iter()
        .map(|b| {
            json!({
                "name": b.name,
                "searchable_concepts": counts.get(b.name.as_str()).copied().unwrap_or(0),
                "declared_okf_version": b.declared_version,
                "conformant": b.conformant,
                "imported_at": b.imported_at,
            })
        })
        .collect()
}

async fn read_concept(
    cg: &TokenSave,
    knowledge: &knowledge::Knowledge,
    bundle: Option<&str>,
    id: &str,
) -> Result<Value> {
    let matches = knowledge.find(bundle, id);
    let concept = match matches.as_slice() {
        [one] => *one,
        [] => {
            return Ok(json!({
                "error": format!("no imported concept `{id}`{}", bundle.map(|b| format!(" in bundle `{b}`")).unwrap_or_default()),
                "hint": "search with `query` to find concept IDs",
            }))
        }
        many => {
            return Ok(json!({
                "error": format!("concept `{id}` exists in several bundles; pass `bundle`"),
                "bundles": many.iter().map(|c| c.bundle.as_str()).collect::<Vec<_>>(),
            }))
        }
    };
    let body_chars = concept.body.chars().count();
    let body: String = concept.body.chars().take(MAX_BODY_CHARS).collect();
    let links: Vec<Value> = concept
        .links
        .iter()
        .take(MAX_LISTED_LINKS)
        .map(|target| json!({ "id": target, "exists": knowledge.exists(&concept.bundle, target) }))
        .collect();
    let backlinks: Vec<&str> = knowledge
        .backlinks(concept)
        .take(MAX_LISTED_LINKS)
        .map(|c| c.id.as_str())
        .collect();
    Ok(json!({
        "bundle": concept.bundle,
        "id": concept.id,
        "type": concept.concept_type,
        "title": concept.title,
        "frontmatter": concept.frontmatter,
        "body": body,
        "body_truncated": body_chars > MAX_BODY_CHARS,
        "links": links,
        "backlinks": backlinks,
        "code_files": code_files(cg, concept).await,
    }))
}

/// Handles `tokensave_knowledge`.
pub(super) async fn handle_knowledge(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    // Reading bundles is blocking file I/O; keep it off the async workers.
    let root = cg.project_root().to_path_buf();
    let knowledge = tokio::task::spawn_blocking(move || knowledge::load(&root))
        .await
        .map_err(|e| TokenSaveError::Config {
            message: format!("loading imported OKF bundles failed: {e}"),
        })??;
    if knowledge.bundles.is_empty() {
        return Ok(text_result(&json!({
            "bundles": [],
            "message": "No OKF bundles are imported in this project. Import one with \
                        `tokensave okf import <dir>`.",
        })));
    }
    let bundle = str_arg(&args, "bundle");
    if let Some(name) = bundle {
        if !knowledge.bundles.iter().any(|b| b.name == name) {
            return Err(TokenSaveError::Config {
                message: format!(
                    "no imported bundle `{name}`; imported: {}",
                    knowledge
                        .bundles
                        .iter()
                        .map(|b| b.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
    }
    if let Some(id) = str_arg(&args, "concept") {
        return Ok(text_result(
            &read_concept(cg, &knowledge, bundle, id).await?,
        ));
    }

    let query = str_arg(&args, "query").unwrap_or_default();
    let file = str_arg(&args, "file").map(|f| f.trim_start_matches("./").replace('\\', "/"));
    let filter = Filter {
        bundle,
        concept_type: str_arg(&args, "type"),
        file: file.as_deref(),
    };
    if query.is_empty()
        && filter.concept_type.is_none()
        && filter.file.is_none()
        && bundle.is_none()
    {
        return Ok(text_result(&json!({
            "bundles": bundles_summary(&knowledge),
            "hint": "pass `query` to search concepts, `file` to find concepts about a \
                     project file, or `concept` to read one",
        })));
    }
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_LIMIT, |n| usize::try_from(n).unwrap_or(MAX_LIMIT))
        .clamp(1, MAX_LIMIT);
    let hits = knowledge.search(query, &filter, limit);
    let mut results = Vec::with_capacity(hits.len());
    for hit in &hits {
        let c = hit.concept;
        results.push(json!({
            "bundle": c.bundle,
            "id": c.id,
            "type": c.concept_type,
            "title": c.title,
            "description": c.description,
            "tags": c.tags,
            "status": c.status,
            "score": hit.score,
            "matched_terms": hit.matched,
            "snippet": hit.snippet,
            "code_files": code_files(cg, c).await,
        }));
    }
    Ok(text_result(&json!({
        "query": query,
        "terms": knowledge::terms(query),
        "results": results,
        "hint": "read a result in full with `concept` (and `bundle`)",
    })))
}
