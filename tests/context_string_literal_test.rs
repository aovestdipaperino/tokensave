//! An exact quoted phrase must survive ranking against generic semantic decoys.
//!
//! Regression test for #362. A UI string lives in the source as a literal and in
//! no symbol name, so FTS can only reach it through whatever field the extractor
//! happened to store it in. When that field is a large object literal — a
//! localization catalog — BM25's length normalization scores the unique, exact
//! hit *below* hundreds of short generic matches, and the single most specific
//! piece of evidence in the query ranks last. The reported symptom was a context
//! request quoting a UI string and getting back nothing but decoy functions.
//!
//! The fix routes multi-word phrases to the existing exact-source scan, which
//! outranks everything. These tests pin down that a quoted phrase reaches the
//! catalog, and that ordinary single-word queries do not pay for a source scan.

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

/// Builds the shape from #362: one catalog holding the phrase, one component
/// referencing it only by key, and many short decoys that match the generic
/// words of the task ("dashboard", "status", "loading").
async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/status-panel.tsx"),
        r#"import { translate } from "./translations";

export function Banner({ busy }: { busy: boolean }): string {
  return translate(busy ? "dashboard.waiting" : "dashboard.ready");
}
"#,
    )
    .unwrap();

    // The phrase sits inside a large catalog, which is what makes BM25 bury it.
    let mut catalog = String::from(
        "export const MESSAGES = {\n  \"dashboard.waiting\": \"Waiting for status\",\n",
    );
    for i in 0..800 {
        catalog.push_str(&format!(
            "  \"catalog.entry.{i}\": \"Synthetic message number {i}\",\n"
        ));
    }
    catalog.push_str("} as const;\n\nexport function translate(key: string): string {\n  return MESSAGES[key];\n}\n");
    fs::write(project.join("src/translations.ts"), catalog).unwrap();

    for file in 0..8 {
        let mut body = String::new();
        for f in 0..30 {
            let n = file * 30 + f;
            body.push_str(&format!(
                "export function loadDashboardStatus{n}(loading: boolean): string {{\n  return loading ? \"Checking dashboard state {n}\" : \"Dashboard ready {n}\";\n}}\n\n"
            ));
        }
        fs::write(project.join(format!("src/decoy-{file}.ts")), body).unwrap();
    }

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn entry_point_names(cg: &TokenSave, task: &str, keywords: &[&str]) -> Vec<String> {
    let options = tokensave::types::BuildContextOptions {
        max_nodes: 20,
        extra_keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
        ..Default::default()
    };
    let context = cg.build_context(task, &options).await.unwrap();
    context
        .entry_points
        .iter()
        .map(|node| node.name.clone())
        .collect()
}

#[tokio::test]
async fn a_quoted_phrase_reaches_the_catalog_that_contains_it() {
    let (_dir, cg) = fixture().await;
    let names = entry_point_names(
        &cg,
        "Diagnose why the dashboard still shows Waiting for status after loading completes.",
        &["dashboard", "Waiting for status", "loading", "status"],
    )
    .await;

    assert!(
        names.contains(&"MESSAGES".to_string()),
        "the catalog holding the exact phrase must be retrieval evidence, got: {names:?}"
    );
}

#[tokio::test]
async fn the_exact_match_outranks_the_generic_decoys() {
    // Being present but 19th is no better than absent when a caller trims the
    // list — the whole failure was generic matches displacing exact evidence.
    let (_dir, cg) = fixture().await;
    let names = entry_point_names(
        &cg,
        "Diagnose why the dashboard still shows Waiting for status after loading completes.",
        &["dashboard", "Waiting for status", "loading", "status"],
    )
    .await;

    assert_eq!(
        names.first().map(String::as_str),
        Some("MESSAGES"),
        "exact copy evidence must rank first, got: {names:?}"
    );
}

#[tokio::test]
async fn a_phrase_quoted_in_the_task_works_without_a_keyword() {
    // Quoting inside the task text is the natural way to ask this, and must not
    // require the caller to also know about the keywords parameter.
    let (_dir, cg) = fixture().await;
    let names = entry_point_names(
        &cg,
        r#"Why does the dashboard still show "Waiting for status" when loading is done?"#,
        &[],
    )
    .await;

    assert!(
        names.contains(&"MESSAGES".to_string()),
        "a phrase quoted in the task must reach the catalog, got: {names:?}"
    );
}

#[tokio::test]
async fn single_word_keywords_do_not_pull_in_the_catalog() {
    // The source scan is the expensive path; a generic one-word query must keep
    // taking the ordinary FTS route rather than matching the catalog's 800 rows.
    let (_dir, cg) = fixture().await;
    let names = entry_point_names(
        &cg,
        "Trace the dashboard loading status logic.",
        &["dashboard", "loading", "status"],
    )
    .await;

    assert!(
        !names.is_empty(),
        "a generic query must still return something"
    );
    assert!(
        !names.contains(&"MESSAGES".to_string()),
        "no exact phrase was given, so the catalog must not be scanned in: {names:?}"
    );
}

#[tokio::test]
async fn search_surfaces_the_catalog_for_an_exact_phrase() {
    // `tokensave_search` has no exact-source channel, so before this the only
    // symbol in the codebase containing the phrase did not appear at all: the
    // result list is sorted by kind tier first, and a `const` can never
    // outrank a function however well it scores (#362).
    let (_dir, cg) = fixture().await;
    let hits = cg.search("Waiting for status", 10).await.unwrap();
    let names: Vec<&str> = hits.iter().map(|h| h.node.name.as_str()).collect();

    assert_eq!(
        names.first(),
        Some(&"MESSAGES"),
        "the one symbol containing the phrase must lead; got {names:?}"
    );
    assert!(
        names.len() > 1,
        "promoting the phrase hit must not discard the other results; got {names:?}"
    );
}

#[tokio::test]
async fn single_word_search_ranking_is_unchanged() {
    // The phrase channel must not disturb ordinary name lookup, which is what
    // search is overwhelmingly used for.
    let (_dir, cg) = fixture().await;
    let hits = cg.search("loadDashboardStatus23", 5).await.unwrap();
    assert_eq!(
        hits.first().map(|h| h.node.name.as_str()),
        Some("loadDashboardStatus23"),
        "exact name match must still win for a single-word query"
    );
}

#[tokio::test]
async fn search_results_do_not_carry_an_unbounded_signature() {
    // The catalog's signature is ~44 KB. Emitted verbatim it blew the tool's
    // 15,000-char response budget, and the truncation cut mid-string, so
    // `tokensave_search` returned *invalid JSON* to its caller.
    let (_dir, cg) = fixture().await;
    let hits = cg.search("Waiting for status", 10).await.unwrap();
    let catalog = hits
        .iter()
        .find(|h| h.node.name == "MESSAGES")
        .expect("catalog must be present");
    let rendered = tokensave::context::compact_signature(
        catalog.node.signature.as_deref().unwrap_or_default(),
    );
    assert!(
        rendered.len() < 300,
        "signature must be bounded before serialization, got {} bytes",
        rendered.len()
    );
}
