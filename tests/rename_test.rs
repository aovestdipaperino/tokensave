//! `tokensave_rename` (#568): graph-based rename with confidence classes.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::config::Toolset;
use tokensave::mcp::handle_tool_call;
use tokensave::mcp::tools::{get_listed_tool_definitions, get_tool_definitions, tool_area};
use tokensave::tokensave::TokenSave;

const UTILS_RS: &str = "/// Computes the total.
pub fn compute_total(x: i32) -> i32 {
    x + 1
}

pub fn other_fn() -> i32 {
    2
}
";

const MAIN_RS: &str = "mod utils;
use crate::utils::compute_total;

fn main() {
    // compute_total is called here
    let v = compute_total(1);
    let w = utils::compute_total(2);
    let s = \"compute_total\";
    println!(\"{} {} {}\", v, w, s);
}
";

async fn rust_project() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("utils.rs"), UTILS_RS).unwrap();
    fs::write(root.join("src").join("main.rs"), MAIN_RS).unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn python_project() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(
        root.join("app.py"),
        "class Greeter:\n    def greet(self):\n        return \"hi\"\n\n\ndef run(g):\n    return g.greet()\n",
    )
    .unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn node_id(cg: &TokenSave, name: &str) -> String {
    let nodes = cg.get_nodes_by_name(name).await.unwrap();
    nodes
        .iter()
        .find(|n| n.kind.as_str() != "use")
        .unwrap_or_else(|| panic!("no node named {name}"))
        .id
        .clone()
}

async fn call(cg: &TokenSave, tool: &str, args: Value) -> Value {
    let result = handle_tool_call(cg, tool, args, None, None).await.unwrap();
    let text = result.value["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
}

/// All non-text sites as `(file, line, confidence, kind)`.
fn sites(plan: &Value) -> Vec<(String, u64, String, String)> {
    let mut out = Vec::new();
    for file in plan["files"].as_array().unwrap() {
        for site in file["sites"].as_array().unwrap() {
            out.push((
                file["file"].as_str().unwrap().to_string(),
                site["line"].as_u64().unwrap(),
                site["confidence"].as_str().unwrap().to_string(),
                site["kind"].as_str().unwrap().to_string(),
            ));
        }
    }
    out
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

#[tokio::test]
async fn dry_run_classifies_sites_and_previews_the_diff() {
    let (dir, cg) = rust_project().await;
    let id = node_id(&cg, "compute_total").await;
    let plan = call(
        &cg,
        "tokensave_rename",
        json!({"node_id": id, "new_name": "sum_total"}),
    )
    .await;

    assert_eq!(plan["dry_run"], true);
    let sites = sites(&plan);
    // Definition, the bare call, the `utils::` path call and the `use`.
    assert!(sites.contains(&(
        "src/utils.rs".into(),
        2,
        "exact".into(),
        "definition".into()
    )));
    assert!(
        sites.contains(&("src/main.rs".into(), 6, "exact".into(), "calls".into())),
        "{sites:?}"
    );
    assert!(
        sites.contains(&("src/main.rs".into(), 7, "exact".into(), "calls".into())),
        "{sites:?}"
    );
    assert!(
        sites.contains(&("src/main.rs".into(), 2, "exact".into(), "uses".into())),
        "{sites:?}"
    );
    assert_eq!(plan["counts"]["exact"], 4, "{plan:#}");
    assert_eq!(plan["counts"]["heuristic"], 0, "{plan:#}");

    // The comment and the string literal are listed, not edited.
    let text_only = plan["text_only"].as_array().unwrap();
    let contexts: BTreeSet<&str> = text_only
        .iter()
        .map(|t| t["kind"].as_str().unwrap())
        .collect();
    assert!(contexts.contains("comment"), "{text_only:?}");
    assert!(contexts.contains("string"), "{text_only:?}");

    let diff = plan["diff"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap())
        .collect::<String>();
    assert!(diff.contains("--- a/src/main.rs"), "{diff}");
    assert!(diff.contains("+    let v = sum_total(1);"), "{diff}");
    assert!(
        diff.contains("+pub fn sum_total(x: i32) -> i32 {"),
        "{diff}"
    );
    assert!(
        !diff.contains("+    let s = \"sum_total\";"),
        "strings are never edited: {diff}"
    );

    // A dry run writes nothing.
    assert_eq!(read(dir.path(), "src/main.rs"), MAIN_RS);
    assert_eq!(read(dir.path(), "src/utils.rs"), UTILS_RS);
}

#[tokio::test]
async fn apply_edits_exact_sites_and_reindexes() {
    let (dir, cg) = rust_project().await;
    let plan = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "compute_total", "new_name": "sum_total", "dry_run": false}),
    )
    .await;
    assert_eq!(plan["applied"], true, "{plan:#}");

    let main = read(dir.path(), "src/main.rs");
    assert!(main.contains("use crate::utils::sum_total;"), "{main}");
    assert!(main.contains("let v = sum_total(1);"), "{main}");
    assert!(main.contains("let w = utils::sum_total(2);"), "{main}");
    // Comments and strings are left for a human.
    assert!(main.contains("// compute_total is called here"), "{main}");
    assert!(main.contains("\"compute_total\""), "{main}");
    assert!(read(dir.path(), "src/utils.rs").contains("pub fn sum_total(x: i32)"));

    // The edited files were reindexed.
    assert!(!cg.get_nodes_by_name("sum_total").await.unwrap().is_empty());
    assert!(cg
        .get_nodes_by_name("compute_total")
        .await
        .unwrap()
        .iter()
        .all(|n| n.kind.as_str() == "use"));
}

#[tokio::test]
async fn heuristic_sites_refuse_apply_without_allow_heuristic() {
    let (dir, cg) = python_project().await;
    let id = node_id(&cg, "greet").await;
    let original = read(dir.path(), "app.py");

    let plan = call(
        &cg,
        "tokensave_rename",
        json!({"node_id": id, "new_name": "salute"}),
    )
    .await;
    let sites = sites(&plan);
    assert!(
        sites.contains(&("app.py".into(), 2, "exact".into(), "definition".into())),
        "{sites:?}"
    );
    // `g.greet()` is bound by the trailing segment of a dotted receiver.
    assert!(
        sites.contains(&("app.py".into(), 7, "heuristic".into(), "calls".into())),
        "{sites:?}"
    );

    let refused = call(
        &cg,
        "tokensave_rename",
        json!({"node_id": id, "new_name": "salute", "dry_run": false}),
    )
    .await;
    assert_eq!(refused["applied"], false, "{refused:#}");
    assert!(refused["refused"]
        .as_str()
        .unwrap()
        .contains("allow_heuristic"));
    let blocking = refused["blocking_sites"].as_array().unwrap();
    assert!(blocking.iter().any(|s| s["line"] == 7), "{blocking:?}");
    assert_eq!(read(dir.path(), "app.py"), original, "nothing is written");

    let applied = call(
        &cg,
        "tokensave_rename",
        json!({"node_id": id, "new_name": "salute", "dry_run": false, "allow_heuristic": true}),
    )
    .await;
    assert_eq!(applied["applied"], true, "{applied:#}");
    let after = read(dir.path(), "app.py");
    assert!(after.contains("def salute(self):"), "{after}");
    assert!(after.contains("return g.salute()"), "{after}");
}

#[tokio::test]
async fn a_reserved_word_is_refused_and_nothing_is_written() {
    let (dir, cg) = rust_project().await;
    // `fn` has an identifier's shape, and tree-sitter-rust parses `fn fn()`
    // without an error node, so it must be refused up front. The
    // all-or-nothing parse check is covered by the `commit_edits` unit tests.
    let result = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "compute_total", "new_name": "fn", "dry_run": false}),
    )
    .await;
    assert_eq!(result["applied"], false, "{result:#}");
    let refused = result["refused"].as_str().unwrap();
    assert!(refused.contains("reserved word in Rust"), "{refused}");
    assert_eq!(read(dir.path(), "src/main.rs"), MAIN_RS);
    assert_eq!(read(dir.path(), "src/utils.rs"), UTILS_RS);
}

#[tokio::test]
async fn a_collision_in_the_same_scope_is_refused() {
    let (dir, cg) = rust_project().await;
    let result = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "compute_total", "new_name": "other_fn", "dry_run": false}),
    )
    .await;
    assert_eq!(result["applied"], false, "{result:#}");
    let blockers = result["blockers"].as_array().unwrap();
    assert!(
        blockers
            .iter()
            .any(|b| b.as_str().unwrap().contains("already names")),
        "{blockers:?}"
    );
    assert_eq!(read(dir.path(), "src/utils.rs"), UTILS_RS);
}

#[tokio::test]
async fn an_invalid_identifier_is_refused() {
    let (_dir, cg) = rust_project().await;
    let result = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "compute_total", "new_name": "sum-total", "dry_run": false}),
    )
    .await;
    assert_eq!(result["applied"], false);
    assert!(result["refused"]
        .as_str()
        .unwrap()
        .contains("not an identifier"));
}

#[tokio::test]
async fn rename_preview_is_a_hidden_dry_run_alias() {
    let (dir, cg) = rust_project().await;
    let id = node_id(&cg, "compute_total").await;

    // Callable, and it never edits even when asked to.
    let plan = call(
        &cg,
        "tokensave_rename_preview",
        json!({"node_id": id, "new_name": "sum_total", "dry_run": false}),
    )
    .await;
    assert_eq!(plan["dry_run"], true, "{plan:#}");
    assert_eq!(plan["counts"]["exact"], 4, "{plan:#}");
    assert!(plan.get("applied").is_none());
    assert_eq!(read(dir.path(), "src/main.rs"), MAIN_RS);

    // Known to dispatch and permissions, absent from every tools/list.
    assert!(get_tool_definitions()
        .iter()
        .any(|d| d.name == "tokensave_rename_preview"));
    for toolset in [Toolset::Full, Toolset::Core] {
        let all: BTreeSet<String> = ["all".to_string()].into();
        let listed = get_listed_tool_definitions(toolset, &all);
        assert!(listed.iter().all(|d| d.name != "tokensave_rename_preview"));
    }
    let listed = get_listed_tool_definitions(Toolset::Full, &BTreeSet::new());
    assert!(listed.iter().any(|d| d.name == "tokensave_rename"));
    assert_eq!(tool_area("tokensave_rename"), "edit");
    // Permission lists keep both names, so an allowlisted alias still works.
    let installable = tokensave::mcp::tools::get_installable_tool_definitions();
    for name in ["tokensave_rename", "tokensave_rename_preview"] {
        assert!(installable.iter().any(|d| d.name == name), "{name}");
    }
}

#[tokio::test]
async fn rename_description_states_its_limits() {
    let def = get_tool_definitions()
        .into_iter()
        .find(|d| d.name == "tokensave_rename")
        .unwrap();
    assert!(def.description.contains("NOT binding-aware"));
    for class in ["`exact`", "`heuristic`", "`ambiguous`", "`text_only`"] {
        assert!(def.description.contains(class), "{class}");
    }
    assert_eq!(def.annotations.unwrap()["readOnlyHint"], false);
}

#[tokio::test]
async fn unknown_node_reports_not_found() {
    let (_dir, cg) = rust_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rename",
        json!({"node_id": "nonexistent_id_12345", "new_name": "x"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = result.value["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Node not found"), "{text}");
}

async fn project(files: &[(&str, &str)]) -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    for (path, body) in files {
        let abs = dir.path().join(path);
        fs::create_dir_all(abs.parent().unwrap()).unwrap();
        fs::write(abs, body).unwrap();
    }
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn caller_names(cg: &TokenSave, name: &str) -> Vec<String> {
    let id = node_id(cg, name).await;
    let mut out: Vec<String> = Vec::new();
    for edge in cg.get_incoming_edges(&id).await.unwrap() {
        if let Some(n) = cg.get_node(&edge.source).await.unwrap() {
            out.push(n.name);
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn callers_survive_a_cross_file_rename() {
    let (dir, cg) = project(&[
        ("lib.rs", "pub fn inc() -> u32 { 1 }\n"),
        ("c.rs", "pub fn caller() -> u32 { inc() }\n"),
    ])
    .await;
    assert_eq!(caller_names(&cg, "inc").await, vec!["caller".to_string()]);

    let applied = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "inc", "new_name": "incr", "dry_run": false}),
    )
    .await;
    assert_eq!(applied["applied"], true, "{applied:#}");
    assert_eq!(
        read(dir.path(), "c.rs"),
        "pub fn caller() -> u32 { incr() }\n"
    );
    assert_eq!(caller_names(&cg, "incr").await, vec!["caller".to_string()]);

    // A sync changes nothing, and a second rename sees the call as exact.
    cg.sync().await.unwrap();
    assert_eq!(caller_names(&cg, "incr").await, vec!["caller".to_string()]);
    let plan = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "incr", "new_name": "increment"}),
    )
    .await;
    let sites = sites(&plan);
    assert!(
        sites.contains(&("c.rs".into(), 1, "exact".into(), "calls".into())),
        "{sites:?}"
    );
}

#[tokio::test]
async fn unlinked_identifiers_past_the_listing_cap_still_gate_apply() {
    // 250 comment mentions in a file sorted first push the local `bump` in
    // z.rs past the 200-entry listing cap.
    let notes: String = (0..250).map(|_| "// bump\n").collect();
    let (dir, cg) = project(&[
        ("a_notes.rs", notes.as_str()),
        (
            "lib.rs",
            "pub fn bump() -> u32 { 1 }\npub fn caller() -> u32 { bump() }\n",
        ),
        (
            "z.rs",
            "pub fn z() -> u32 {\n    let bump = 5;\n    bump + 1\n}\n",
        ),
    ])
    .await;
    let result = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "bump", "new_name": "bumped", "dry_run": false}),
    )
    .await;
    assert_eq!(result["applied"], false, "{result:#}");
    assert!(
        result["unlinked_code_omitted"].as_u64().unwrap() >= 2,
        "{result:#}"
    );
    assert!(
        result["refused"].as_str().unwrap().contains("not exact"),
        "{result:#}"
    );
    assert!(read(dir.path(), "lib.rs").contains("pub fn bump()"));
}

#[tokio::test]
async fn a_file_too_large_to_scan_gates_apply() {
    let (dir, cg) = project(&[
        (
            "lib.rs",
            "pub fn bump() -> u32 { 1 }\npub fn caller() -> u32 { bump() }\n",
        ),
        ("z.rs", "pub fn z() -> u32 { 1 }\n"),
    ])
    .await;
    // Grows past the scan limit after indexing, with a mention at the end.
    let mut big = "// filler line\n".repeat(150_000);
    big.push_str("pub fn y() -> u32 { bump() }\n");
    fs::write(dir.path().join("z.rs"), big).unwrap();

    let result = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "bump", "new_name": "bumped", "dry_run": false}),
    )
    .await;
    assert_eq!(result["applied"], false, "{result:#}");
    let unscanned = result["unscanned"].as_array().unwrap();
    assert!(
        unscanned[0].as_str().unwrap().starts_with("z.rs"),
        "{unscanned:?}"
    );
    assert!(result["refused"]
        .as_str()
        .unwrap()
        .contains("could not be checked"));
    assert!(read(dir.path(), "lib.rs").contains("pub fn bump()"));
}

#[tokio::test]
async fn columns_are_byte_columns_and_crlf_tabs_and_utf8_survive() {
    let caller = "pub fn caller() -> usize {\r\n\tlet s = \"é€😀\"; compute(s)\r\n}\r\n";
    let (dir, cg) = project(&[
        ("lib.rs", "pub fn compute(s: &str) -> usize { s.len() }\r\n"),
        ("c.rs", caller),
    ])
    .await;
    let plan = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "compute", "new_name": "measure"}),
    )
    .await;
    let site = plan["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file"] == "c.rs")
        .unwrap_or_else(|| panic!("{plan:#}"))["sites"][0]
        .clone();
    // `\t` (1) + `let s = "` (9) + `é€😀` (2 + 3 + 4 bytes) + `"; ` (3) = 22
    // bytes before the name: byte column 23, where a char column would be 17.
    assert_eq!(site["line"], 2, "{site}");
    assert_eq!(site["column"], 23, "{site}");
    assert_eq!(site["end_column"], 30, "{site}");

    let applied = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "compute", "new_name": "measure", "dry_run": false}),
    )
    .await;
    assert_eq!(applied["applied"], true, "{applied:#}");
    assert_eq!(
        read(dir.path(), "c.rs"),
        caller.replace("compute", "measure")
    );
    assert_eq!(
        read(dir.path(), "lib.rs"),
        "pub fn measure(s: &str) -> usize { s.len() }\r\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_file_is_edited_through_its_target() {
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("c_target.rs");
    fs::write(&target, "pub fn caller() -> u32 { inc() }\n").unwrap();
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn inc() -> u32 { 1 }\n").unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join("c.rs")).unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();

    let applied = call(
        &cg,
        "tokensave_rename",
        json!({"symbol": "inc", "new_name": "incr", "dry_run": false}),
    )
    .await;
    assert_eq!(applied["applied"], true, "{applied:#}");
    let link = dir.path().join("c.rs");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the link must stay a link"
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "pub fn caller() -> u32 { incr() }\n"
    );
}
