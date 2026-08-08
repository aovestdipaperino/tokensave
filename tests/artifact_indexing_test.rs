//! Non-code artifacts must be findable by path.
//!
//! Regression test for #323. `tokensave_files` accepted a glob pattern, but the
//! `files` table was populated only by the symbol-extraction pass, so anything
//! without extractable symbols — `.feature` specs, fixtures, schemas — was
//! absent. The pattern was valid and matched nothing. With the shell fallback
//! blocked by the hook, "where are the .feature files for the login flow?" had
//! no answer at all.
//!
//! Artifacts are tracked by path only. They are never parsed, contribute no
//! symbols, and stay distinguishable from source so analyses meaning "code"
//! do not silently widen.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::FileKind;

/// The layout from the issue: a source file, a spec, a schema, and a fixture.
async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("features")).unwrap();

    fs::write(
        project.join("src/login.rs"),
        "pub fn submit_credentials() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        project.join("features/login.feature"),
        "Feature: Login\n  Scenario: Valid credentials\n    Given the login form is open\n",
    )
    .unwrap();
    fs::write(
        project.join("features/checkout.feature"),
        "Feature: Checkout\n",
    )
    .unwrap();
    fs::write(project.join("schema.json"), "{\"title\": \"account\"}\n").unwrap();
    fs::write(project.join("data.bin"), "not a tracked artifact\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn paths(cg: &TokenSave) -> Vec<String> {
    let mut out: Vec<String> = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn feature_files_are_indexed() {
    // The reported symptom: the glob was valid, the entries were absent.
    let (_dir, cg) = fixture().await;
    let paths = paths(&cg).await;
    assert!(
        paths.contains(&"features/login.feature".to_string()),
        "the .feature file must be discoverable by path, got: {paths:?}"
    );
}

#[tokio::test]
async fn artifacts_are_distinguishable_from_source() {
    // A source file with no symbols is a fact about the code; an artifact was
    // never parsed. Collapsing the two would make "0 symbols" ambiguous and
    // would let analyses meaning "code" widen without anyone noticing.
    let (_dir, cg) = fixture().await;
    let files = cg.get_all_files().await.unwrap();

    let spec = files
        .iter()
        .find(|f| f.path == "features/login.feature")
        .expect("spec must be indexed");
    assert_eq!(spec.kind, FileKind::Artifact);
    assert_eq!(spec.node_count, 0, "artifacts are never parsed");

    let source = files
        .iter()
        .find(|f| f.path == "src/login.rs")
        .expect("source must be indexed");
    assert_eq!(source.kind, FileKind::Code);
}

#[tokio::test]
async fn artifacts_carry_a_hash_and_stat_like_any_other_file() {
    // Incremental sync partitions on (mtime, size) and then on content hash.
    // An artifact row missing them would be re-processed on every single sync.
    let (_dir, cg) = fixture().await;
    let files = cg.get_all_files().await.unwrap();
    let spec = files
        .iter()
        .find(|f| f.path == "features/login.feature")
        .unwrap();

    assert!(
        !spec.content_hash.is_empty(),
        "artifact needs a content hash"
    );
    assert!(spec.size > 0, "artifact needs a size");
    assert!(spec.modified_at > 0, "artifact needs an mtime");
}

#[tokio::test]
async fn unlisted_extensions_are_still_ignored() {
    // The default set is deliberately narrow. Tracking every text file would
    // turn the file list into a directory listing.
    let (_dir, cg) = fixture().await;
    let paths = paths(&cg).await;
    assert!(
        !paths.contains(&"data.bin".to_string()),
        "an unlisted extension must not be tracked, got: {paths:?}"
    );
}

#[tokio::test]
async fn code_only_queries_exclude_artifacts() {
    // Source-text scanners read every file they are given; handing them the
    // project's schemas and fixtures is pure cost and invites false positives.
    let (_dir, cg) = fixture().await;
    let code: Vec<String> = cg
        .get_code_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();

    assert!(code.contains(&"src/login.rs".to_string()));
    assert!(
        !code.iter().any(|p| p.ends_with(".feature")),
        "artifacts must not appear in a code-only query, got: {code:?}"
    );
}

#[tokio::test]
async fn sync_adds_a_new_artifact() {
    // A spec added after the initial index must appear without a full reindex.
    let (dir, cg) = fixture().await;
    fs::write(
        dir.path().join("features/signup.feature"),
        "Feature: Signup\n",
    )
    .unwrap();

    cg.sync().await.unwrap();

    let paths = paths(&cg).await;
    assert!(
        paths.contains(&"features/signup.feature".to_string()),
        "sync must pick up a new artifact, got: {paths:?}"
    );
}

#[tokio::test]
async fn sync_removes_a_deleted_artifact() {
    // Removal runs off the same DB-vs-disk comparison as source, so this pins
    // that artifacts were not accidentally exempted from it.
    let (dir, cg) = fixture().await;
    fs::remove_file(dir.path().join("features/checkout.feature")).unwrap();

    cg.sync().await.unwrap();

    let paths = paths(&cg).await;
    assert!(
        !paths.contains(&"features/checkout.feature".to_string()),
        "sync must drop a deleted artifact, got: {paths:?}"
    );
}

#[tokio::test]
async fn an_edited_artifact_gets_a_new_hash() {
    // Without this the row would go stale silently, and every consumer that
    // trusts the hash to mean "current contents" would be wrong.
    let (dir, cg) = fixture().await;
    let before = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.path == "features/login.feature")
        .unwrap()
        .content_hash;

    fs::write(
        dir.path().join("features/login.feature"),
        "Feature: Login\n  Scenario: Invalid credentials\n",
    )
    .unwrap();
    cg.sync().await.unwrap();

    let after = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.path == "features/login.feature")
        .unwrap()
        .content_hash;

    assert_ne!(before, after, "an edited artifact must be re-hashed");
}

#[tokio::test]
async fn an_extension_owned_by_an_extractor_stays_source() {
    // `.md` ships in the default artifact set. If a markdown extractor is ever
    // registered (#154), the symbol pass owns those files and the artifact pass
    // must yield rather than race it to write the same row.
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn f() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let source = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.path == "src/lib.rs")
        .expect("source must be indexed");
    assert_eq!(source.kind, FileKind::Code);
    assert!(source.node_count > 0, "source must still be parsed");
}
