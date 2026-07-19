//! Focused regression tests for `tokensave_simplify_scan`.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

fn simplify_output_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

async fn index_two_receiver_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub mod alpha;\npub mod beta;\n",
    )
    .unwrap();
    fs::write(
        project.join("src/alpha.rs"),
        r#"
pub struct Alpha;

impl Alpha {
    pub fn from_json(value: &str) -> Option<Self> {
        (!value.is_empty()).then_some(Self)
    }

    pub fn canonical_json(&self) -> &'static str {
        "alpha-canonical"
    }

    pub fn identity_preimage_json(&self) -> &'static str {
        "alpha-preimage"
    }

    pub fn computed_id(&self) -> u64 {
        11
    }

    pub fn computed_digest(&self) -> u64 {
        alpha_digest(self)
    }

    pub fn seal(&self) -> bool {
        alpha_seal(self)
    }

    pub fn validate(&self) -> bool {
        alpha_validate(self)
    }
}

fn alpha_digest(_: &Alpha) -> u64 { 101 }
fn alpha_seal(_: &Alpha) -> bool { true }
fn alpha_validate(_: &Alpha) -> bool { true }
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/beta.rs"),
        r#"
pub struct Beta;

impl Beta {
    pub fn from_json(value: &str) -> Option<Self> {
        value.starts_with('{').then_some(Self)
    }

    pub fn canonical_json(&self) -> &'static str {
        "beta-canonical"
    }

    pub fn identity_preimage_json(&self) -> &'static str {
        "beta-preimage"
    }

    pub fn computed_id(&self) -> u64 {
        22
    }

    pub fn computed_digest(&self) -> u64 {
        beta_digest(self)
    }

    pub fn seal(&self) -> bool {
        beta_seal(self)
    }

    pub fn validate(&self) -> bool {
        beta_validate(self)
    }
}

fn beta_digest(_: &Beta) -> u64 { 202 }
fn beta_seal(_: &Beta) -> bool { false }
fn beta_validate(_: &Beta) -> bool { false }
"#,
    )
    .unwrap();

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    (graph, dir)
}

async fn index_exact_clone_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub mod alpha;\npub mod beta;\npub mod unrelated;\n",
    )
    .unwrap();
    let implementation = r#"
    pub fn validate(&self, value: i32) -> bool {
        value > 0 && value % 2 == 0
    }
"#;
    fs::write(
        project.join("src/alpha.rs"),
        format!("pub struct Alpha;\nimpl Alpha {{{implementation}}}\n"),
    )
    .unwrap();
    fs::write(
        project.join("src/beta.rs"),
        format!("pub struct Beta;\nimpl Beta {{{implementation}}}\n"),
    )
    .unwrap();
    fs::write(
        project.join("src/unrelated.rs"),
        "pub fn untouched() -> u8 { 0 }\n",
    )
    .unwrap();

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    (graph, dir)
}

async fn index_empty_body_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub mod alpha;\npub mod beta;\n",
    )
    .unwrap();
    fs::write(
        project.join("src/alpha.rs"),
        "pub struct Alpha;\nimpl Alpha { pub fn validate(&self) {} }\n",
    )
    .unwrap();
    fs::write(
        project.join("src/beta.rs"),
        "pub struct Beta;\nimpl Beta { pub fn validate(&self) {} }\n",
    )
    .unwrap();

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    (graph, dir)
}

async fn index_renamed_clone_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub mod alpha;\npub mod beta;\n",
    )
    .unwrap();
    fs::write(
        project.join("src/alpha.rs"),
        r#"
pub fn normalize(values: &[i32]) -> i32 {
    let mut total = 0;
    for value in values {
        total += value;
    }
    total
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/beta.rs"),
        r#"
pub fn normalize_copy(items: &[i32]) -> i32 {
    let mut sum = 0;
    for item in items {
        sum += item;
    }
    sum
}
"#,
    )
    .unwrap();

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    (graph, dir)
}

async fn index_reformatted_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub mod alpha;\npub mod beta;\n",
    )
    .unwrap();
    fs::write(
        project.join("src/alpha.rs"),
        "pub struct Alpha;\nimpl Alpha {\n    pub fn validate(&self, value: i32) -> bool {\n        value > 0\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/beta.rs"),
        "pub struct Beta;\nimpl Beta { pub fn validate(&self, value: i32) -> bool { value > 0 } }\n",
    )
    .unwrap();

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    (graph, dir)
}

async fn index_large_exact_name_project(reverse_creation_order: bool) -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub mod candidates;\npub mod target;\n",
    )
    .unwrap();

    let mut candidates = String::new();
    for index in 0..240 {
        candidates.push_str(&format!(
            "pub struct Candidate{index};\nimpl Candidate{index} {{\n    pub fn validate(&self) -> usize {{\n        {index}\n    }}\n}}\n"
        ));
    }
    let target =
        "pub struct Target;\nimpl Target {\n    pub fn validate(&self) -> usize {\n        233\n    }\n}\n";
    if reverse_creation_order {
        fs::write(project.join("src/target.rs"), target).unwrap();
        fs::write(project.join("src/candidates.rs"), candidates).unwrap();
    } else {
        fs::write(project.join("src/candidates.rs"), candidates).unwrap();
        fs::write(project.join("src/target.rs"), target).unwrap();
    }

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();

    (graph, dir)
}

#[tokio::test]
async fn same_named_inherent_methods_on_distinct_receivers_are_not_duplicates() {
    let (graph, _dir) = index_two_receiver_project().await;
    let result = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(simplify_output_text(&result.value)).unwrap();

    assert_eq!(output["duplications"], json!([]), "{output:#}");
}

#[tokio::test]
async fn exact_copied_method_body_reports_qualified_hash_evidence_once() {
    let (graph, _dir) = index_exact_clone_project().await;
    let result = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(simplify_output_text(&result.value)).unwrap();
    let findings = output["duplications"].as_array().unwrap();

    assert_eq!(findings.len(), 1, "{output:#}");
    let finding = &findings[0];
    assert_eq!(finding["symbol"], "validate");
    assert!(finding["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(finding["qualified_name"], "src/alpha.rs::Alpha::validate");
    let matches = finding["similar_to"].as_array().unwrap();
    assert_eq!(matches.len(), 1, "{output:#}");
    let duplicate = &matches[0];
    assert!(duplicate["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(duplicate["qualified_name"], "src/beta.rs::Beta::validate");
    assert_eq!(duplicate["score"], 1.0);
    assert_eq!(duplicate["evidence_kind"], "exact_source_body_hash");
    assert!(duplicate["body_hash"]
        .as_str()
        .is_some_and(|hash| hash.len() == 16));
}

#[tokio::test]
async fn identical_empty_method_bodies_are_not_duplication_evidence() {
    let (graph, _dir) = index_empty_body_project().await;
    let result = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(simplify_output_text(&result.value)).unwrap();

    assert_eq!(output["duplications"], json!([]), "{output:#}");
}

#[tokio::test]
async fn renamed_clone_is_delegated_to_redundancy_tool() {
    let (graph, _dir) = index_renamed_clone_project().await;
    let simplify = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let simplify_output: Value =
        serde_json::from_str(simplify_output_text(&simplify.value)).unwrap();
    assert_eq!(
        simplify_output["duplications"],
        json!([]),
        "{simplify_output:#}"
    );

    let redundancy = handle_tool_call(
        &graph,
        "tokensave_redundancy",
        json!({"path": "src", "min_lines": 3, "similarity_threshold": 0.8}),
        None,
        None,
    )
    .await
    .unwrap();
    let redundancy_output: Value =
        serde_json::from_str(simplify_output_text(&redundancy.value)).unwrap();
    let pairs = redundancy_output["pairs"].as_array().unwrap();
    assert!(
        pairs.iter().any(|pair| {
            let names = [pair["a"]["name"].as_str(), pair["b"]["name"].as_str()];
            names.contains(&Some("normalize")) && names.contains(&Some("normalize_copy"))
        }),
        "{redundancy_output:#}"
    );
}

#[tokio::test]
async fn repeated_runs_and_reordered_duplicate_inputs_are_byte_identical() {
    let (graph, _dir) = index_exact_clone_project().await;
    let canonical = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let reordered = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/beta.rs", "src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let repeated = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        simplify_output_text(&canonical.value),
        simplify_output_text(&reordered.value)
    );
    assert_eq!(
        simplify_output_text(&canonical.value),
        simplify_output_text(&repeated.value)
    );
}

#[tokio::test]
async fn reformatted_near_duplicate_is_not_an_exact_body_match() {
    let (graph, _dir) = index_reformatted_project().await;
    let result = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/alpha.rs", "src/beta.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(simplify_output_text(&result.value)).unwrap();

    assert_eq!(output["duplications"], json!([]), "{output:#}");
}

#[tokio::test]
async fn exact_clone_is_found_when_the_rest_of_the_diff_is_unrelated() {
    let (graph, _dir) = index_exact_clone_project().await;
    let result = handle_tool_call(
        &graph,
        "tokensave_simplify_scan",
        json!({"files": ["src/unrelated.rs", "src/alpha.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(simplify_output_text(&result.value)).unwrap();
    let findings = output["duplications"].as_array().unwrap();

    assert_eq!(findings.len(), 1, "{output:#}");
    assert_eq!(
        findings[0]["qualified_name"],
        "src/alpha.rs::Alpha::validate"
    );
    assert_eq!(
        findings[0]["similar_to"][0]["qualified_name"],
        "src/beta.rs::Beta::validate"
    );
}

#[tokio::test]
async fn exact_clone_beyond_two_hundred_candidates_is_complete_and_deterministic() {
    let (forward, _forward_dir) = index_large_exact_name_project(false).await;
    let (reverse, _reverse_dir) = index_large_exact_name_project(true).await;

    let forward_result = handle_tool_call(
        &forward,
        "tokensave_simplify_scan",
        json!({"files": ["src/target.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let reverse_result = handle_tool_call(
        &reverse,
        "tokensave_simplify_scan",
        json!({"files": ["src/target.rs"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let forward_output: Value =
        serde_json::from_str(simplify_output_text(&forward_result.value)).unwrap();
    let reverse_output: Value =
        serde_json::from_str(simplify_output_text(&reverse_result.value)).unwrap();

    assert_eq!(forward_output["duplications"].as_array().unwrap().len(), 1);
    assert_eq!(
        forward_output["duplications"][0]["similar_to"][0]["qualified_name"],
        "src/candidates.rs::Candidate233::validate"
    );
    assert_eq!(forward_output, reverse_output);
}
