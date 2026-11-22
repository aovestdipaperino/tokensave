//! `tokensave_knowledge`: search and read imported OKF bundles (#858,
//! #1060).

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
const DEFAULT_RELATED_DEPTH: usize = 1;
const MAX_RELATED_DEPTH: usize = 3;

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
            let r = &b.record;
            json!({
                "name": r.name,
                "title": b.meta.title,
                "description": b.meta.description,
                "license": b.meta.license,
                "searchable_concepts": counts.get(r.name.as_str()).copied().unwrap_or(0),
                "declared_okf_version": b.meta.okf_version,
                "read_mode": b.read_mode,
                "conformant": r.conformant,
                "imported_at": r.imported_at,
            })
        })
        .collect()
}

/// Trust, freshness and lifecycle of a concept, as reported with each
/// result (OKF §5.3 to §5.5).
fn signals(knowledge: &knowledge::Knowledge, concept: &Concept, now: i64) -> Value {
    let mut out = json!({
        "status": concept.status,
        "current": concept.is_current(),
        "trust_tier": concept.trust.tier,
        "freshness": concept.freshness(now),
    });
    if let Some(superseded_by) = &concept.superseded_by {
        out["superseded_by"] = json!(superseded_by);
        out["replacement"] = json!(knowledge.replacement(concept).map(|c| c.id.as_str()));
    }
    if let Some(gaps) = &concept.attestation {
        out["attestable"] = json!(gaps.is_empty());
    }
    out
}

fn merge(mut base: Value, extra: Value) -> Value {
    if let (Some(base), Value::Object(extra)) = (base.as_object_mut(), extra) {
        base.extend(extra);
    }
    base
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
    let field_links: Vec<Value> = concept
        .field_links
        .iter()
        .take(MAX_LISTED_LINKS)
        .map(|link| json!({ "field": link.field, "id": link.id }))
        .collect();
    let backlinks: Vec<&str> = knowledge
        .backlinks(concept)
        .take(MAX_LISTED_LINKS)
        .map(|c| c.id.as_str())
        .collect();
    let mut out = json!({
        "bundle": concept.bundle,
        "id": concept.id,
        "type": concept.concept_type,
        "title": concept.title,
        "frontmatter": concept.frontmatter,
        "body": body,
        "body_truncated": body_chars > MAX_BODY_CHARS,
        "links": links,
        "field_links": field_links,
        "backlinks": backlinks,
        "code_files": code_files(cg, concept).await,
        "trust": concept.trust,
    });
    out = merge(out, signals(knowledge, concept, crate::okf::time::now()));
    if let Some(gaps) = &concept.attestation {
        out["attestation_gaps"] = json!(gaps);
    }
    Ok(out)
}

async fn related_concepts(
    cg: &TokenSave,
    knowledge: &knowledge::Knowledge,
    bundle: Option<&str>,
    id: &str,
    depth: usize,
    limit: usize,
) -> Result<Value> {
    let matches = knowledge.find(bundle, id);
    let start = match matches.as_slice() {
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
    let related = knowledge.related(start, depth, limit);
    let mut results = Vec::with_capacity(related.len());
    for item in related {
        let concept = item.concept;
        results.push(json!({
            "bundle": concept.bundle,
            "id": concept.id,
            "type": concept.concept_type,
            "title": concept.title,
            "distance": item.distance,
            "status": concept.status,
            "trust_tier": concept.trust.tier,
            "code_files": code_files(cg, concept).await,
        }));
    }
    Ok(json!({
        "bundle": start.bundle,
        "concept": start.id,
        "depth": depth,
        "results": results,
        "hint": "read a related concept in full with `concept` (and `bundle`)",
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
        if !knowledge.bundles.iter().any(|b| b.record.name == name) {
            return Err(TokenSaveError::Config {
                message: format!(
                    "no imported bundle `{name}`; imported: {}",
                    knowledge
                        .bundles
                        .iter()
                        .map(|b| b.record.name.as_str())
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

    if let Some(id) = str_arg(&args, "related") {
        let depth = args
            .get("depth")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_RELATED_DEPTH, |n| {
                usize::try_from(n).unwrap_or(MAX_RELATED_DEPTH)
            })
            .clamp(1, MAX_RELATED_DEPTH);
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_LIMIT, |n| usize::try_from(n).unwrap_or(MAX_LIMIT))
            .clamp(1, MAX_LIMIT);
        return Ok(text_result(
            &related_concepts(cg, &knowledge, bundle, id, depth, limit).await?,
        ));
    }

    let query = str_arg(&args, "query").unwrap_or_default();
    let file = str_arg(&args, "file").map(|f| f.trim_start_matches("./").replace('\\', "/"));
    let filter = Filter {
        bundle,
        concept_type: str_arg(&args, "type"),
        file: file.as_deref(),
        current_only: args
            .get("current_only")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    if query.is_empty()
        && filter.concept_type.is_none()
        && filter.file.is_none()
        && bundle.is_none()
    {
        return Ok(text_result(&json!({
            "bundles": bundles_summary(&knowledge),
            "hint": "pass `query` to search concepts, `file` to find concepts about a \
                     project file, `concept` to read one, or `related` to traverse links",
        })));
    }
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_LIMIT, |n| usize::try_from(n).unwrap_or(MAX_LIMIT))
        .clamp(1, MAX_LIMIT);
    let hits = knowledge.search(query, &filter, limit);
    let now = crate::okf::time::now();
    let mut results = Vec::with_capacity(hits.len());
    for hit in &hits {
        let c = hit.concept;
        let mut result = json!({
            "bundle": c.bundle,
            "id": c.id,
            "type": c.concept_type,
            "title": c.title,
            "description": c.description,
            "tags": c.tags,
            "score": hit.score,
            "matched_terms": hit.matched,
            "snippet": hit.snippet,
            "code_files": code_files(cg, c).await,
        });
        result = merge(result, signals(&knowledge, c, now));
        if let Some(old) = hit.replaces {
            result["replaces"] = json!(old.id);
        }
        results.push(result);
    }
    Ok(text_result(&json!({
        "query": query,
        "terms": knowledge::terms(query),
        "results": results,
        "hint": "read a result in full with `concept` (and `bundle`)",
    })))
}
