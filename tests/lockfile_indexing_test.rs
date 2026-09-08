//! Terraform/OpenTofu dependency lockfiles must be indexed as artifacts.
//!
//! `.terraform.lock.hcl` (the basename Terraform and OpenTofu share) holds the
//! resolved provider source addresses, selected versions, and checksums of an
//! infrastructure tree. It is committed by convention, but the scanner dropped
//! it twice over: the hidden-entry filter skips dot-prefixed names, and no
//! extractor or artifact extension owns `.hcl`, so sync reported it as an
//! unsupported file and neither `tokensave_files` nor literal search could
//! reach it.
//!
//! The lockfile is classified by exact basename as a path-tracked artifact:
//! never parsed, so checksum strings cannot surface as symbols, and other
//! HCL documents (Packer, Consul, Vault) stay unsupported rather than being
//! mislabelled as Terraform source.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::tokensave::TokenSave;
use tokensave::types::FileKind;

const LOCKFILE: &str = r#"# This file is maintained automatically by "terraform init".
# Manual edits may be lost in future updates.

provider "registry.terraform.io/hashicorp/aws" {
  version     = "5.82.2"
  constraints = "~> 5.0"
  hashes = [
    "h1:examplehashAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    "zh:0a8efb3775a13613bce5270696ce350947ebdf1ce5bf956aea95923db0a30036",
  ]
}

provider "registry.opentofu.org/timofurrer/desec" {
  version     = "0.6.2"
  constraints = "0.6.2"
  hashes = [
    "h1:otherexamplehashBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=",
  ]
}
"#;

/// A project with Terraform source, a nested lockfile, and a non-lock HCL
/// file, indexed with the default (gitignore-aware) scanner.
async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("infra/terraform")).unwrap();
    fs::create_dir_all(project.join("packer")).unwrap();

    fs::write(
        project.join("infra/terraform/versions.tf"),
        "terraform {\n  required_version = \">= 1.6\"\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("infra/terraform/.terraform.lock.hcl"),
        LOCKFILE,
    )
    .unwrap();
    fs::write(
        project.join("packer/consul.hcl"),
        "acl {\n  enabled = true\n}\n",
    )
    .unwrap();

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
async fn lockfile_is_indexed_despite_hidden_name_and_unsupported_extension() {
    // The reported symptom: sync succeeded, the skipped summary counted one
    // unsupported `.hcl` file, and the lockfile was absent from the file list.
    let (_dir, cg) = fixture().await;
    let paths = paths(&cg).await;
    assert!(
        paths.contains(&"infra/terraform/.terraform.lock.hcl".to_string()),
        "the lockfile must be discoverable by path, got: {paths:?}"
    );
}

#[tokio::test]
async fn lockfile_is_an_artifact_with_no_symbols() {
    // Artifact, not source: parsed HCL would turn provider checksums into
    // nodes, and a lockfile has no symbols to begin with.
    let (_dir, cg) = fixture().await;
    let files = cg.get_all_files().await.unwrap();
    let lock = files
        .iter()
        .find(|f| f.path == "infra/terraform/.terraform.lock.hcl")
        .expect("lockfile must be indexed");
    assert_eq!(lock.kind, FileKind::Artifact);
    assert_eq!(lock.node_count, 0, "a lockfile is never parsed");
    assert!(!lock.content_hash.is_empty());
    assert!(lock.size > 0);
    assert!(lock.modified_at > 0);
}

#[tokio::test]
async fn non_lock_hcl_files_stay_unsupported() {
    // Basename classification, not extension claiming: a Consul/Packer-style
    // `.hcl` document must not become an artifact or a source file, and the
    // unsupported-extension accounting must keep seeing it.
    let (_dir, cg) = fixture().await;
    let paths = paths(&cg).await;
    assert!(
        !paths.contains(&"packer/consul.hcl".to_string()),
        "a non-lock .hcl file must not be tracked, got: {paths:?}"
    );

    let result = cg.sync().await.unwrap();
    assert!(
        result
            .skipped_extensions
            .iter()
            .any(|(ext, count)| ext == "hcl" && *count == 1),
        "the non-lock .hcl file must still count as skipped, got: {:?}",
        result.skipped_extensions
    );
    // Exactly one `.hcl` file exists on disk (the non-lock one) and it alone
    // accounts for the count: the lockfile must not add to it.
}

#[tokio::test]
async fn lockfile_content_is_searchable_by_literal_scan() {
    // Literal search reads every indexed file's bytes, so both registry
    // address forms must be findable once the row exists.
    let (_dir, cg) = fixture().await;
    let files = cg.get_all_files().await.unwrap();
    let lock = files
        .iter()
        .find(|f| f.path == "infra/terraform/.terraform.lock.hcl")
        .unwrap();
    let source = fs::read_to_string(cg.project_root().join(&lock.path)).unwrap();
    assert!(source.contains("registry.terraform.io/hashicorp/aws"));
    assert!(source.contains("registry.opentofu.org/timofurrer/desec"));
}

#[tokio::test]
async fn sync_picks_up_a_newly_written_lockfile() {
    // Projects that ran `terraform init` after their first index must gain
    // the lockfile on an incremental sync, not just a full reindex.
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("infra/terraform")).unwrap();
    fs::write(
        project.join("infra/terraform/versions.tf"),
        "terraform {}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    assert!(
        !paths(&cg)
            .await
            .contains(&"infra/terraform/.terraform.lock.hcl".to_string()),
        "no lockfile yet"
    );

    fs::write(
        project.join("infra/terraform/.terraform.lock.hcl"),
        LOCKFILE,
    )
    .unwrap();
    cg.sync().await.unwrap();

    let paths = paths(&cg).await;
    assert!(
        paths.contains(&"infra/terraform/.terraform.lock.hcl".to_string()),
        "sync must pick up the new lockfile, got: {paths:?}"
    );
}

#[tokio::test]
async fn lockfile_is_found_without_gitignore_support() {
    // The walkdir scanner (git_ignore = false) applies its own hidden-entry
    // filter; the lockfile exemption must hold there too.
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("infra/terraform")).unwrap();
    fs::write(
        project.join("infra/terraform/.terraform.lock.hcl"),
        LOCKFILE,
    )
    .unwrap();

    TokenSave::init(project).await.unwrap();
    let mut config = load_config(project).unwrap();
    config.git_ignore = false;
    save_config(project, &config).unwrap();
    let cg = TokenSave::open(project).await.unwrap();
    cg.index_all().await.unwrap();

    let paths = paths(&cg).await;
    assert!(
        paths.contains(&"infra/terraform/.terraform.lock.hcl".to_string()),
        "walkdir scan must find the lockfile, got: {paths:?}"
    );
}

#[tokio::test]
async fn other_hidden_files_remain_excluded() {
    // The lockfile exemption must not widen hidden-file handling generally.
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("infra/terraform")).unwrap();
    fs::write(
        project.join("infra/terraform/.terraform.lock.hcl"),
        LOCKFILE,
    )
    .unwrap();
    fs::write(project.join("infra/terraform/.env"), "SECRET=nope\n").unwrap();
    fs::write(project.join("infra/terraform/.notes.md"), "hidden\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let paths = paths(&cg).await;
    assert!(paths.contains(&"infra/terraform/.terraform.lock.hcl".to_string()));
    assert!(
        !paths.contains(&"infra/terraform/.env".to_string()),
        "hidden files must stay excluded, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"infra/terraform/.notes.md".to_string()),
        "hidden files must stay excluded even with a tracked extension, got: {paths:?}"
    );
}
