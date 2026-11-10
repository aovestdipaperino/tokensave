//! `tokensave storage switch` (#946).
//!
//! A build can only open its own backend's index, so each half of a switch
//! is tested in the build that runs it: `prepare` saves this build's records
//! for the other backend, and `finish` completes a switch prepared for this
//! build. The switch file is the only interface between the halves, so a
//! file written by `prepare` and addressed back to this build exercises the
//! same format the other build reads. Run with `--features grafeo` to cover
//! the Grafeo side.

use std::path::Path;

use tempfile::TempDir;
use tokensave::storage_switch::{self, StorageBackend};
use tokensave::tokensave::TokenSave;

const CURRENT: StorageBackend = StorageBackend::current();

fn write_sources(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src").join("lib.rs"),
        "pub fn alpha() -> u32 { beta() + 1 }\n\
         pub fn beta() -> u32 { 41 }\n\
         pub struct Gamma { pub value: u32 }\n\
         impl Gamma { pub fn get(&self) -> u32 { alpha() + self.value } }\n",
    )
    .expect("write lib.rs");
    std::fs::write(
        root.join("src").join("main.rs"),
        "fn main() { let g = Gamma { value: 1 }; println!(\"{}\", g.get()); }\n",
    )
    .expect("write main.rs");
}

async fn indexed_project() -> (TempDir, TokenSave) {
    let dir = TempDir::new().expect("tempdir");
    write_sources(dir.path());
    let ts = TokenSave::init(dir.path()).await.expect("init");
    ts.index_all().await.expect("index");
    (dir, ts)
}

/// Graph shape that a fresh `init` of the same tree must reproduce.
async fn graph_shape(ts: &TokenSave) -> (u64, u64, u64, Vec<(String, u64)>, Vec<(String, u64)>) {
    let stats = ts.get_stats().await.expect("stats");
    let mut nodes: Vec<_> = stats.nodes_by_kind.into_iter().collect();
    nodes.sort();
    let mut edges: Vec<_> = stats.edges_by_kind.into_iter().collect();
    edges.sort();
    (
        stats.node_count,
        stats.edge_count,
        stats.file_count,
        nodes,
        edges,
    )
}

async fn record_memory(ts: &TokenSave) {
    let db = ts.db();
    db.insert_decision(
        "use a builder for Gamma",
        Some("keeps the constructor stable"),
        &["src/lib.rs".to_string()],
        &["design".to_string()],
        1_700_000_000,
    )
    .await
    .expect("decision 1");
    db.insert_decision("no reason given", None, &[], &[], 1_700_000_100)
        .await
        .expect("decision 2");
    db.upsert_code_area("src/lib.rs", Some("core API"), 1_700_000_200)
        .await
        .expect("area 1");
    db.upsert_code_area("src/lib.rs", None, 1_700_000_300)
        .await
        .expect("area 1 again");
    db.upsert_code_area("src/main.rs", None, 1_700_000_400)
        .await
        .expect("area 2");
}

/// Rewrites a prepared switch file as if this build's backend were the
/// other one, so this build can finish it.
fn address_to_this_build(root: &Path) {
    let path = storage_switch::handoff_path(root);
    let mut handoff: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read switch file"))
            .expect("parse switch file");
    handoff["from"] = serde_json::json!(CURRENT.other().as_str());
    handoff["to"] = serde_json::json!(CURRENT.as_str());
    std::fs::write(&path, handoff.to_string()).expect("write switch file");
}

#[tokio::test]
async fn prepare_saves_records_and_leaves_the_index_unchanged() {
    let (dir, ts) = indexed_project().await;
    record_memory(&ts).await;
    let before = graph_shape(&ts).await;
    drop(ts);

    let report = storage_switch::prepare(dir.path(), CURRENT.other(), false)
        .await
        .expect("prepare");
    assert_eq!(report.from, CURRENT);
    assert_eq!(report.to, CURRENT.other());
    assert_eq!(report.decisions, 2);
    assert_eq!(report.code_areas, 2);

    let handoff: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(storage_switch::handoff_path(dir.path())).expect("read"),
    )
    .expect("parse");
    assert_eq!(handoff["to"], CURRENT.other().as_str());
    assert!(handoff["config"].is_object(), "config is carried over");
    let areas = handoff["code_areas"].as_array().expect("areas");
    let lib = areas
        .iter()
        .find(|a| a["path"] == "src/lib.rs")
        .expect("lib.rs area");
    assert_eq!(lib["touch_count"], 2);
    assert_eq!(lib["description"], "core API");
    assert_eq!(lib["last_touched_at"], 1_700_000_300);

    // The source index is still usable and unchanged.
    let reopened = TokenSave::open(dir.path()).await.expect("reopen");
    assert_eq!(graph_shape(&reopened).await, before);
    let decisions = reopened
        .db()
        .get_decisions(None, None, -1)
        .await
        .expect("decisions");
    assert_eq!(decisions.len(), 2);
}

#[tokio::test]
async fn prepare_refuses_a_project_without_an_index() {
    let dir = TempDir::new().expect("tempdir");
    let error = storage_switch::prepare(dir.path(), CURRENT.other(), false)
        .await
        .expect_err("nothing to switch");
    assert!(error.to_string().contains("nothing to switch"), "{error}");
    assert!(!storage_switch::handoff_path(dir.path()).exists());
}

#[tokio::test]
async fn finish_rebuilds_the_index_and_restores_records() {
    let (dir, ts) = indexed_project().await;
    record_memory(&ts).await;
    drop(ts);
    storage_switch::prepare(dir.path(), CURRENT.other(), false)
        .await
        .expect("prepare");
    address_to_this_build(dir.path());

    // A stand-in for the other backend's index, which the switch removes.
    let old_index = storage_switch::index_path(dir.path(), CURRENT.other());
    std::fs::create_dir_all(old_index.parent().expect("parent")).expect("old dir");
    std::fs::write(&old_index, b"old index").expect("old index");

    let (report, ts) = storage_switch::finish(dir.path(), false, |_, _, _| {})
        .await
        .expect("finish");
    assert_eq!(report.to, CURRENT);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(report.kept_old.is_none());
    assert!(!old_index.exists(), "old index removed");
    assert!(!storage_switch::handoff_path(dir.path()).exists());

    // Records are present once each, even though the rebuilt index already
    // held the same records.
    let decisions = ts
        .db()
        .get_decisions(None, None, -1)
        .await
        .expect("decisions");
    assert_eq!(decisions.len(), 2);
    let builder = decisions
        .iter()
        .find(|d| d.text == "use a builder for Gamma")
        .expect("decision kept");
    assert_eq!(
        builder.reason.as_deref(),
        Some("keeps the constructor stable")
    );
    assert_eq!(builder.files, vec!["src/lib.rs".to_string()]);
    assert_eq!(builder.tags, vec!["design".to_string()]);
    assert_eq!(builder.created_at, 1_700_000_000);
    let areas = ts.db().get_code_areas(-1).await.expect("areas");
    assert_eq!(areas.len(), 2);
    let lib = areas
        .iter()
        .find(|a| a.path == "src/lib.rs")
        .expect("lib.rs");
    assert_eq!(lib.touch_count, 2);
    assert_eq!(lib.description.as_deref(), Some("core API"));
    assert_eq!(lib.last_touched_at, 1_700_000_300);

    // The rebuilt graph matches a fresh init of the same tree.
    let (fresh_dir, fresh) = indexed_project().await;
    assert_eq!(graph_shape(&ts).await, graph_shape(&fresh).await);
    drop(fresh_dir);
}

#[tokio::test]
async fn finish_keeps_the_old_index_when_asked() {
    let (dir, ts) = indexed_project().await;
    drop(ts);
    storage_switch::prepare(dir.path(), CURRENT.other(), true)
        .await
        .expect("prepare");
    address_to_this_build(dir.path());
    let old_index = storage_switch::index_path(dir.path(), CURRENT.other());
    std::fs::create_dir_all(old_index.parent().expect("parent")).expect("old dir");
    std::fs::write(&old_index, b"old index").expect("old index");

    let (report, _ts) = storage_switch::finish(dir.path(), false, |_, _, _| {})
        .await
        .expect("finish");
    assert_eq!(
        report.kept_old,
        Some(storage_switch::index_dir(dir.path(), CURRENT.other()))
    );
    assert!(old_index.exists(), "--keep-old given when preparing");
}

#[tokio::test]
async fn an_interrupted_finish_can_run_again() {
    let dir = TempDir::new().expect("tempdir");
    write_sources(dir.path());
    // Switch file from the other build, and its index, still intact.
    let handoff = serde_json::json!({
        "format": 1,
        "from": CURRENT.other().as_str(),
        "to": CURRENT.as_str(),
        "created_at": 1,
        "decisions": [{ "created_at": 5, "text": "from the other backend" }],
        "code_areas": [{ "path": "src/main.rs", "last_touched_at": 6, "touch_count": 3 }],
    });
    let handoff_path = storage_switch::handoff_path(dir.path());
    std::fs::create_dir_all(handoff_path.parent().expect("parent")).expect("channel dir");
    std::fs::write(&handoff_path, handoff.to_string()).expect("switch file");
    let old_index = storage_switch::index_path(dir.path(), CURRENT.other());
    std::fs::create_dir_all(old_index.parent().expect("parent")).expect("old dir");
    std::fs::write(&old_index, b"old index").expect("old index");

    // An earlier finish stopped part way: the target index exists but holds
    // only some files and one record of its own.
    {
        let partial = TokenSave::init(dir.path()).await.expect("init");
        partial
            .db()
            .insert_decision("written by this build", None, &[], &[], 7)
            .await
            .expect("decision");
    }
    assert!(old_index.exists(), "the original index is untouched so far");
    assert!(storage_switch::storage_hint(dir.path()).is_some_and(|hint| hint.contains("prepared")));

    let (report, ts) = storage_switch::finish(dir.path(), false, |_, _, _| {})
        .await
        .expect("finish again");
    assert_eq!(report.decisions, 2);
    assert_eq!(report.code_areas, 1);
    let mut texts: Vec<String> = ts
        .db()
        .get_decisions(None, None, -1)
        .await
        .expect("decisions")
        .into_iter()
        .map(|d| d.text)
        .collect();
    texts.sort();
    assert_eq!(texts, ["from the other backend", "written by this build"]);
    let (fresh_dir, fresh) = indexed_project().await;
    assert_eq!(graph_shape(&ts).await, graph_shape(&fresh).await);
    drop(fresh_dir);
    assert!(storage_switch::storage_hint(dir.path()).is_none());
}

#[tokio::test]
async fn finish_without_a_prepared_switch_explains_the_first_step() {
    let dir = TempDir::new().expect("tempdir");
    let old_index = storage_switch::index_path(dir.path(), CURRENT.other());
    std::fs::create_dir_all(old_index.parent().expect("parent")).expect("old dir");
    std::fs::write(&old_index, b"old index").expect("old index");

    let error = match storage_switch::switch(dir.path(), CURRENT, false, |_, _, _| {}).await {
        Ok(_) => panic!("no switch was prepared"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains(&format!("tokensave storage switch {CURRENT}")),
        "{error}"
    );
    assert!(old_index.exists());
}

#[tokio::test]
async fn finish_refuses_a_switch_addressed_to_the_other_backend() {
    let (dir, ts) = indexed_project().await;
    drop(ts);
    storage_switch::prepare(dir.path(), CURRENT.other(), false)
        .await
        .expect("prepare");
    let error = match storage_switch::finish(dir.path(), false, |_, _, _| {}).await {
        Ok(_) => panic!("the switch targets the other backend"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains(CURRENT.other().label()), "{error}");
    assert!(storage_switch::handoff_path(dir.path()).exists());
    assert!(storage_switch::storage_hint(dir.path())
        .is_some_and(|hint| hint.contains(&format!("with a {} build", CURRENT.other().label()))));
}

#[tokio::test]
async fn a_newer_index_of_the_other_backend_is_hinted() {
    let (dir, ts) = indexed_project().await;
    drop(ts);
    assert!(storage_switch::storage_hint(dir.path()).is_none());

    let own = storage_switch::index_path(dir.path(), CURRENT);
    let other = storage_switch::index_path(dir.path(), CURRENT.other());
    std::fs::create_dir_all(other.parent().expect("parent")).expect("other dir");
    std::fs::write(&other, b"other index").expect("other index");
    let own_time = std::fs::metadata(&own)
        .and_then(|m| m.modified())
        .expect("own mtime");
    std::fs::File::options()
        .write(true)
        .open(&other)
        .and_then(|f| f.set_modified(own_time + std::time::Duration::from_secs(3600)))
        .expect("set mtime");

    let hint = storage_switch::storage_hint(dir.path()).expect("hint");
    assert!(hint.contains("newer"), "{hint}");
    assert!(
        hint.contains(&format!("tokensave storage switch {CURRENT}")),
        "{hint}"
    );
}
