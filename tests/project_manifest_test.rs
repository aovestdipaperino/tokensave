//! Integration tests for `.tokensave/project.json` (#194): explicit index
//! entries with per-entry language overrides, including extensionless
//! dotfiles, wrong-extension globs, and paths outside the project root.

use std::fs;
use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

const BASH_PROFILE: &str = "export PATH=\"$HOME/bin:$PATH\"\n\nmy_greet() {\n    echo hello\n}\n";
const SHRC: &str = "my_prompt() {\n    echo prompt\n}\n";

async fn index_with_manifest(manifest: &str) -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join(".tokensave")).unwrap();
    fs::create_dir_all(project.join("homedir/.bashrc.d")).unwrap();
    fs::write(project.join("homedir/.bash_profile"), BASH_PROFILE).unwrap();
    fs::write(project.join("homedir/.bashrc.d/prompt.shrc"), SHRC).unwrap();
    // A normal source file to prove default dispatch still works.
    fs::write(project.join("main.sh"), "regular_fn() { echo hi; }\n").unwrap();
    fs::write(project.join(".tokensave/project.json"), manifest).unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn manifest_indexes_extensionless_and_wrong_extension_files_as_bash() {
    let (cg, _dir) = index_with_manifest(
        r#"{
  "version": 1,
  "entries": [
    { "path": "homedir/.bash_profile", "language": "bash" },
    { "path": "homedir/.bashrc.d/*.shrc", "language": "bash" }
  ]
}"#,
    )
    .await;

    let nodes = cg.get_all_nodes().await.unwrap();
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"my_greet"),
        "extensionless .bash_profile not indexed as bash: {names:?}"
    );
    assert!(
        names.contains(&"my_prompt"),
        "*.shrc glob not indexed as bash: {names:?}"
    );
    assert!(
        names.contains(&"regular_fn"),
        "normal extension dispatch broke: {names:?}"
    );
    // Graph paths are the real relative paths, not aliases.
    assert!(nodes.iter().any(|n| n.file_path == "homedir/.bash_profile"));
    assert!(nodes
        .iter()
        .any(|n| n.file_path == "homedir/.bashrc.d/prompt.shrc"));
}

#[tokio::test]
async fn manifest_external_absolute_path_is_indexed() {
    // The "external" file lives in its own temp dir, outside the project.
    let external_dir = TempDir::new().unwrap();
    let external_file = external_dir.path().join(".bash_env");
    fs::write(&external_file, "external_fn() {\n    echo x\n}\n").unwrap();

    let manifest = format!(
        r#"{{ "version": 1, "entries": [ {{ "path": "{}", "language": "bash" }} ] }}"#,
        external_file.to_string_lossy().replace('\\', "/")
    );

    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join(".tokensave")).unwrap();
    fs::write(project.join("main.sh"), "local_fn() { echo hi; }\n").unwrap();
    fs::write(project.join(".tokensave/project.json"), manifest).unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let nodes = cg.get_all_nodes().await.unwrap();
    let external = nodes
        .iter()
        .find(|n| n.name == "external_fn")
        .unwrap_or_else(|| {
            panic!(
                "external file not indexed: {:?}",
                nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
            )
        });
    // Stored under its real resolved (absolute) path.
    assert!(
        external.file_path.ends_with(".bash_env")
            && std::path::Path::new(&external.file_path).is_absolute(),
        "expected absolute real path, got {}",
        external.file_path
    );
    assert!(nodes.iter().any(|n| n.name == "local_fn"));
}

#[tokio::test]
async fn unknown_language_fails_the_sync_loudly() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join(".tokensave")).unwrap();
    fs::write(project.join("main.sh"), "f() { echo hi; }\n").unwrap();
    fs::write(
        project.join(".tokensave/project.json"),
        r#"{ "version": 1, "entries": [ { "path": "x/*.cfg", "language": "klingon" } ] }"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    let msg = match cg.index_all().await {
        Ok(_) => panic!("index_all should fail on an unknown manifest language"),
        Err(e) => e.to_string(),
    };
    assert!(msg.contains("klingon"), "{msg}");
    assert!(msg.contains("valid languages"), "{msg}");
}

#[tokio::test]
async fn sync_picks_up_manifest_files_too() {
    let (cg, dir) = index_with_manifest(
        r#"{ "version": 1, "entries": [ { "path": "homedir/.bashrc.d/*.shrc", "language": "bash" } ] }"#,
    )
    .await;

    // Add a new manifest-matched file after the initial index and sync.
    fs::write(
        dir.path().join("homedir/.bashrc.d/aliases.shrc"),
        "my_alias_fn() {\n    echo a\n}\n",
    )
    .unwrap();
    cg.sync().await.unwrap();

    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        nodes.iter().any(|n| n.name == "my_alias_fn"),
        "sync missed a new manifest-matched file"
    );
}
