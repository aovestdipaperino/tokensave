//! Extension dispatch and configuration revision regressions (Refs #725).
use std::fs;
use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::extraction::LanguageRegistry;
use tokensave::project_manifest::{load_manifest, resolve_extractor};
use tokensave::tokensave::TokenSave;

fn aliases(root: &std::path::Path, pairs: &[(&str, &str)]) {
    let mut config = load_config(root).unwrap();
    config.language_aliases = pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    save_config(root, &config).unwrap();
}

#[test]
fn validates_normalization_conflicts_and_enabled_languages() {
    let dir = TempDir::new().unwrap();
    let registry = LanguageRegistry::new();
    for key in ["", ".", "a.b", "a/b", "a\\b", "*.t", "a b"] {
        aliases(dir.path(), &[(key, "python")]);
        assert!(load_manifest(dir.path(), &registry).is_err(), "{key}");
    }
    aliases(dir.path(), &[(".T", "python"), ("t", "rust")]);
    assert!(load_manifest(dir.path(), &registry)
        .unwrap_err()
        .to_string()
        .contains("conflicting"));
    aliases(dir.path(), &[(".t", "not-a-language")]);
    let err = load_manifest(dir.path(), &registry)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown or disabled") && err.contains("enabled languages"));
    #[cfg(not(feature = "lang-perl"))]
    {
        aliases(dir.path(), &[(".t", "perl")]);
        assert!(load_manifest(dir.path(), &registry).is_err());
    }
    aliases(dir.path(), &[(" .T ", "PyThOn"), ("ep", "python")]);
    for path in ["t/basic.t", "t/UPPER.T", "t\\basic.T", "template.ep"] {
        assert_eq!(
            resolve_extractor(&registry, dir.path(), path)
                .unwrap()
                .language_name(),
            "Python"
        );
    }
}

#[tokio::test]
async fn aliases_patterns_artifacts_and_mapping_revisions() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    aliases(
        root,
        &[
            (".custom", "python"),
            (".json", "python"),
            (".rs", "python"),
        ],
    );
    fs::write(root.join("a.custom"), "def aliased():\n    pass\n").unwrap();
    fs::write(
        root.join("data.json"),
        "def artifact_override():\n    pass\n",
    )
    .unwrap();
    fs::write(root.join("main.rs"), "def native_override():\n    pass\n").unwrap();
    fs::write(root.join("native.py"), "def native():\n    pass\n").unwrap();
    fs::write(root.join("skip.unknown"), "def skipped(): pass\n").unwrap();
    fs::write(root.join("binary.custom"), b"\x00\xff\x00").unwrap();
    fs::write(root.join("ordinary.yaml"), "name: artifact\n").unwrap();
    fs::write(tokensave::config::get_tokensave_dir(root).join("project.json"), r#"{"entries":[{"path":"*.custom","language":"rust"},{"path":"a.custom","language":"python"}]}"#).unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    for name in ["aliased", "artifact_override", "native_override", "native"] {
        assert!(nodes.iter().any(|n| n.name == name), "missing {name}");
    }
    assert!(!nodes.iter().any(|n| n.name == "skipped"));
    assert!(!cg
        .get_all_files()
        .await
        .unwrap()
        .iter()
        .any(|f| f.path == "binary.custom"));
    assert!(cg
        .get_all_files()
        .await
        .unwrap()
        .iter()
        .any(|f| f.path == "ordinary.yaml" && f.kind == tokensave::types::FileKind::Artifact));
    assert_eq!(cg.sync().await.unwrap().files_modified, 0);
    aliases(
        root,
        &[(".custom", "python"), (".json", "python"), (".rs", "rust")],
    );
    cg.sync().await.unwrap();
    assert!(!cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "native_override"));
    // Removing the literal lets the earlier Rust glob win, without editing the file.
    fs::write(
        tokensave::config::get_tokensave_dir(root).join("project.json"),
        r#"{"entries":[{"path":"*.custom","language":"rust"}]}"#,
    )
    .unwrap();
    assert!(!cg.find_stale_files().await.is_empty());
    cg.sync_if_stale_silent(&cg.find_stale_files().await)
        .await
        .unwrap();
    assert!(!cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "aliased"));
    // Alias removal converts JSON to artifact, removing the old symbols.
    aliases(root, &[]);
    fs::remove_file(tokensave::config::get_tokensave_dir(root).join("project.json")).unwrap();
    cg.sync().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(!nodes.iter().any(|n| n.name == "artifact_override"));
    assert!(!cg
        .get_all_files()
        .await
        .unwrap()
        .iter()
        .any(|f| f.path == "a.custom"));
    assert!(cg
        .get_all_files()
        .await
        .unwrap()
        .iter()
        .any(|f| f.path == "data.json" && f.kind == tokensave::types::FileKind::Artifact));
    assert_eq!(cg.sync().await.unwrap().files_modified, 0);
}

#[cfg(feature = "lang-perl")]
#[tokio::test]
async fn legacy_perl_fixture_extracts_native_and_aliases_then_updates_and_deletes() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for path in [
        "lib/Example.pm",
        "t/basic.t",
        "t/integration.t",
        ".tokensave/config.json",
    ] {
        let target = if path == ".tokensave/config.json" {
            tokensave::config::get_tokensave_dir(root).join("config.json")
        } else {
            root.join(path)
        };
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("legacy_perl")
                .join(path),
            target,
        )
        .unwrap();
    }
    fs::write(
        root.join("t").join("template.ep"),
        "sub alias_test { return 1; }\n",
    )
    .unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    let result = cg.index_all().await.unwrap();
    assert!(!result.skipped_extensions.iter().any(|(ext, _)| ext == "t"));
    let nodes = cg.get_all_nodes().await.unwrap();
    for name in ["example", "basic_test", "integration_test", "alias_test"] {
        assert!(nodes.iter().any(|n| n.name == name), "{name}");
    }
    assert_eq!(
        cg.get_stats().await.unwrap().files_by_language.get("Perl"),
        Some(&4)
    );
    assert!(cg
        .search("basic_test", 10)
        .await
        .unwrap()
        .iter()
        .any(|r| r.node.file_path == "t/basic.t"));
    let context = cg
        .build_context(
            "basic_test",
            &tokensave::types::BuildContextOptions::default(),
        )
        .await
        .unwrap();
    assert!(context.entry_points.iter().any(|n| n.name == "basic_test"));
    fs::write(
        root.join("t").join("basic.t"),
        "sub replacement { return 1; }\n",
    )
    .unwrap();
    fs::remove_file(root.join("t").join("integration.t")).unwrap();
    cg.sync().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(nodes.iter().any(|n| n.name == "replacement"));
    assert!(!nodes
        .iter()
        .any(|n| n.name == "integration_test" || n.name == "basic_test"));
    cg.index_all().await.unwrap();
    assert!(cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "replacement"));
}

#[tokio::test]
async fn cli_worker_dispatch_and_unsupported_diagnostics() {
    let dir = TempDir::new().unwrap();
    aliases(dir.path(), &[(".custom", "python")]);
    fs::write(
        dir.path().join("worker.custom"),
        "def worker_symbol():\n    pass\n",
    )
    .unwrap();
    fs::write(dir.path().join("unmapped.oddformat"), "unsupported\n").unwrap();
    let run = |args: &[&str]| {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .args(args)
            .arg(dir.path())
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
    };
    let output = run(&["init"]);
    assert!(
        output.contains("language_aliases") && output.contains("artifact_extensions"),
        "{output}"
    );
    let output = run(&["sync", "--doctor"]);
    assert!(
        output.contains(".oddformat: 1 file(s)") && output.contains("project.json"),
        "{output}"
    );
    assert!(!output.contains(".custom: 1 file(s)"));
    let cg = TokenSave::open(dir.path()).await.unwrap();
    assert!(cg
        .search("worker_symbol", 10)
        .await
        .unwrap()
        .iter()
        .any(|r| r.node.name == "worker_symbol"));
    assert_eq!(
        cg.get_stats()
            .await
            .unwrap()
            .files_by_language
            .get("Python"),
        Some(&1)
    );
}

#[tokio::test]
async fn repeated_sync_preserves_fixed_workload() {
    let dir = TempDir::new().unwrap();
    aliases(dir.path(), &[(".custom", "python")]);
    // A bounded synthetic repository exercises parallel extraction and repeated
    // no-op sync without flaky timing or allocator-dependent RSS assertions.
    for i in 0..256 {
        fs::write(
            dir.path().join(format!("file_{i}.custom")),
            format!("def function_{i}():\n    pass\n"),
        )
        .unwrap();
    }
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    for _ in 0..5 {
        let result = cg.sync().await.unwrap();
        assert_eq!(
            (
                result.files_added,
                result.files_modified,
                result.files_removed
            ),
            (0, 0, 0)
        );
        assert_eq!(cg.get_all_files().await.unwrap().len(), 256);
        assert_eq!(
            cg.get_stats()
                .await
                .unwrap()
                .files_by_language
                .get("Python"),
            Some(&256)
        );
        assert!(cg.find_stale_files().await.is_empty());
    }
}

#[tokio::test]
async fn binary_only_mapping_does_not_parse_or_panic() {
    let dir = TempDir::new().unwrap();
    aliases(dir.path(), &[(".custom", "python")]);
    fs::write(dir.path().join("binary.custom"), b"\0def secret(): pass\0").unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    assert_eq!(cg.index_all().await.unwrap().file_count, 0);
    assert!(cg.get_all_nodes().await.unwrap().is_empty());
}
