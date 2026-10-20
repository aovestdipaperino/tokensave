//! Unknown-extension detection stays separate from extension aliases.
use std::fs;
use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::extraction::LanguageRegistry;
use tokensave::language_detection::{detect, detect_file, DetectionConfig, Method};
use tokensave::tokensave::TokenSave;

fn enabled() -> DetectionConfig {
    DetectionConfig {
        enabled: true,
        ..DetectionConfig::default()
    }
}
const PY: &str = "def detected_name():\n    return 42\n";
const RS: &str = "use std::fmt;\npub fn detected_rust() { println!(\"hello\"); }\n";

#[test]
fn detects_supported_shebangs_and_rejects_ambiguous_names() {
    let registry = LanguageRegistry::new();
    for line in [
        "#!/usr/bin/env python3",
        "#!/usr/bin/env -S python3 -u",
        "#! /usr/bin/python3.12",
        "#!/usr/bin/python",
    ] {
        let d = detect(&registry, format!("{line}\n{PY}").as_bytes(), enabled()).unwrap();
        assert_eq!((d.language, d.method), ("Python", Method::Shebang));
    }
    for line in [
        "#!/usr/bin/env python3evil",
        "#!python3",
        "#!/usr/bin/env -S 'python3 -u'",
        "#!/bin/unknown",
    ] {
        assert!(detect(&registry, format!("{line}\n{PY}").as_bytes(), enabled()).is_none());
    }
    let shell = detect(&registry, b"#!/bin/bash\nf() { echo hi; }\n", enabled());
    assert_eq!(shell.is_some(), cfg!(feature = "lang-bash"));
    #[cfg(feature = "lang-bash")]
    assert_eq!(shell.unwrap().language, "Bash");
}

#[test]
fn distinctive_syntax_requires_multiple_signals_and_a_unique_candidate() {
    let registry = LanguageRegistry::new();
    for (source, language) in [
        (PY, "Python"),
        (RS, "Rust"),
        (
            "import x from 'x';\nexport function f() { return x; }",
            "JavaScript",
        ),
        (
            "export interface X { name: string; }\nexport function f() { return 1; }",
            "TypeScript",
        ),
        ("package main\nfunc main() {\n}\n", "Go"),
        ("#include <stdio.h>\nint main() { return 0; }", "C"),
        ("#include <iostream>\nusing namespace std;", "C++"),
    ] {
        assert_eq!(
            detect(&registry, source.as_bytes(), enabled())
                .unwrap()
                .language,
            language,
            "{source}"
        );
    }
    for source in [
        "",
        "class Example",
        "function f() {}",
        "import x",
        "fn f() {}",
        "def f():",
        "ordinary prose",
    ] {
        assert!(
            detect(&registry, source.as_bytes(), enabled()).is_none(),
            "{source}"
        );
    }
    assert!(detect(&registry, format!("{PY}\n{RS}").as_bytes(), enabled()).is_none());
}

#[test]
fn malformed_binary_sensitive_and_bounded_inputs_fail_safely() {
    let registry = LanguageRegistry::new();
    for bytes in [
        b"\0#!/bin/bash".as_slice(),
        b"\xff\xfe",
        b"def f():\n    return 1\0",
        b"-----BEGIN PRIVATE KEY-----\n",
    ] {
        assert!(detect(&registry, bytes, enabled()).is_none());
    }
    let mut large = vec![b' '; 4096];
    large.extend_from_slice(PY.as_bytes());
    assert!(detect(&registry, &large, enabled()).is_none());
    assert!(detect(&registry, PY.as_bytes(), DetectionConfig::default()).is_none());
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("secret.pem");
    fs::write(&path, PY).unwrap();
    assert!(detect_file(&registry, &path, enabled()).is_none());
    assert!(serde_json::from_str::<DetectionConfig>(r#"{"max_bytes":0}"#).is_err());
    assert!(serde_json::from_str::<DetectionConfig>(r#"{"max_bytes":65537}"#).is_err());
}

#[test]
fn cache_reuses_unchanged_files_and_invalidates_edits_and_settings() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("script.custom");
    let registry = LanguageRegistry::new();
    fs::write(&path, PY).unwrap();
    assert_eq!(
        detect_file(&registry, &path, enabled()).unwrap().language,
        "Python"
    );
    let before = tokensave::language_detection::statistics();
    assert_eq!(
        detect_file(&registry, &path, enabled()).unwrap().language,
        "Python"
    );
    assert!(tokensave::language_detection::statistics().cache_hits > before.cache_hits);
    fs::write(&path, RS).unwrap();
    assert_eq!(
        detect_file(&registry, &path, enabled()).unwrap().language,
        "Rust"
    );
    assert!(detect_file(&registry, &path, DetectionConfig::default()).is_none());
    fs::write(&path, "ambiguous").unwrap();
    assert!(detect_file(&registry, &path, enabled()).is_none());
}

async fn project(enabled_detection: bool) -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    drop(cg);
    let mut config = load_config(dir.path()).unwrap();
    config.language_detection.enabled = enabled_detection;
    save_config(dir.path(), &config).unwrap();
    let cg = TokenSave::open(dir.path()).await.unwrap();
    (dir, cg)
}

#[tokio::test]
async fn indexes_symbols_syncs_edits_deletes_and_forced_reindex() {
    let (dir, cg) = project(true).await;
    fs::write(dir.path().join("script.custom"), PY).unwrap();
    fs::write(
        dir.path().join("native.py"),
        "def native_name():\n    pass\n",
    )
    .unwrap();
    fs::write(dir.path().join("rust.custom"), RS).unwrap();
    cg.index_all().await.unwrap();
    assert!(cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "detected_name" && n.file_path == "script.custom"));
    assert!(!cg.search("detected_name", 10).await.unwrap().is_empty());
    let builder = tokensave::context::ContextBuilder::new(cg.db(), dir.path());
    let context = builder
        .build_context(
            "detected_name",
            &tokensave::types::BuildContextOptions::default(),
        )
        .await
        .unwrap();
    assert!(!context.entry_points.is_empty());
    cg.sync().await.unwrap();
    assert_eq!(cg.sync().await.unwrap().files_modified, 0);
    fs::write(
        dir.path().join("script.custom"),
        "def replacement_longer_name():\n    return 12345\n",
    )
    .unwrap();
    cg.sync().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(nodes.iter().any(|n| n.name == "replacement_longer_name"));
    assert!(!nodes.iter().any(|n| n.name == "detected_name"));
    fs::remove_file(dir.path().join("script.custom")).unwrap();
    cg.sync().await.unwrap();
    assert!(!cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "replacement_longer_name"));
    cg.index_all().await.unwrap();
    assert!(cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "detected_rust"));
}

#[tokio::test]
async fn opt_in_preserves_known_extension_manifest_and_selection_policies() {
    let (dir, cg) = project(false).await;
    fs::write(dir.path().join("script.custom"), PY).unwrap();
    cg.index_all().await.unwrap();
    assert!(!cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "detected_name"));
    drop(cg);
    let mut config = load_config(dir.path()).unwrap();
    config.language_detection.enabled = true;
    config.exclude.push("excluded.*".into());
    config.max_file_size = 1024;
    save_config(dir.path(), &config).unwrap();
    fs::write(dir.path().join("excluded.custom"), PY).unwrap();
    fs::write(
        dir.path().join("huge.custom"),
        format!("{PY}{}", " ".repeat(2048)),
    )
    .unwrap();
    fs::write(dir.path().join("secret.key"), PY).unwrap();
    fs::write(
        dir.path().join("known.rs"),
        format!("#!/usr/bin/env python3\n{PY}"),
    )
    .unwrap();
    fs::write(dir.path().join("override.custom"), PY).unwrap();
    fs::write(
        dir.path().join(".tokensave/project.json"),
        r#"{"version":1,"entries":[{"path":"override.custom","language":"Rust"}]}"#,
    )
    .unwrap();
    let cg = TokenSave::open(dir.path()).await.unwrap();
    cg.sync().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(nodes
        .iter()
        .any(|n| n.file_path == "script.custom" && n.name == "detected_name"));
    assert!(!nodes.iter().any(|n| [
        "excluded.custom",
        "huge.custom",
        "secret.key",
        "known.rs",
        "override.custom"
    ]
    .contains(&n.file_path.as_str())
        && n.name == "detected_name"));
    drop(cg);
    config.language_detection.enabled = false;
    save_config(dir.path(), &config).unwrap();
    let cg = TokenSave::open(dir.path()).await.unwrap();
    cg.sync().await.unwrap();
    assert!(!cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.file_path == "script.custom"));
}

#[tokio::test]
async fn artifacts_are_fallback_and_settings_reclassify_unchanged_files() {
    let (dir, cg) = project(true).await;
    drop(cg);
    let mut config = load_config(dir.path()).unwrap();
    config.artifact_extensions.push("custom".into());
    config.language_detection.max_bytes = 128;
    save_config(dir.path(), &config).unwrap();
    let source = format!("{}\n{PY}", "# padding\n".repeat(20));
    fs::write(dir.path().join("script.custom"), source).unwrap();
    let cg = TokenSave::open(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(
        cg.get_all_files().await.unwrap()[0].kind,
        tokensave::types::FileKind::Artifact
    );
    drop(cg);
    config.language_detection.max_bytes = 4096;
    save_config(dir.path(), &config).unwrap();
    let cg = TokenSave::open(dir.path()).await.unwrap();
    cg.sync().await.unwrap();
    assert!(cg
        .search("detected_name", 10)
        .await
        .unwrap()
        .iter()
        .any(|r| r.node.name == "detected_name"));
    fs::write(dir.path().join("script.custom"), "ordinary text again").unwrap();
    assert!(!cg.sync_if_stale(&["script.custom".into()]).await.unwrap());
    assert!(cg.search("detected_name", 10).await.unwrap().is_empty());
    assert_eq!(
        cg.get_all_files().await.unwrap()[0].kind,
        tokensave::types::FileKind::Artifact
    );
}

fn git(root: &std::path::Path, args: &[&str]) {
    let result = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test]
async fn branch_indexes_keep_detected_symbols_separate() {
    let (dir, cg) = project(true).await;
    drop(cg);
    let root = dir.path();
    git(root, &["init", "-b", "master"]);
    fs::write(root.join("script.custom"), PY).unwrap();
    git(root, &["add", "script.custom"]);
    git(root, &["commit", "-m", "base"]);
    let cg = TokenSave::open(root).await.unwrap();
    cg.index_all().await.unwrap();
    drop(cg);
    git(root, &["checkout", "-b", "feature"]);
    tokensave::branch::track_branch_copy(root, &root.join(".tokensave"), "feature")
        .await
        .unwrap();
    fs::write(root.join("script.custom"), RS).unwrap();
    git(root, &["add", "script.custom"]);
    git(root, &["commit", "-m", "switch language"]);
    let cg = TokenSave::open(root).await.unwrap();
    cg.sync().await.unwrap();
    assert!(cg
        .get_all_nodes()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "detected_rust"));
    drop(cg);
    git(root, &["checkout", "master"]);
    let cg = TokenSave::open(root).await.unwrap();
    cg.sync().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(nodes.iter().any(|n| n.name == "detected_name"));
    assert!(!nodes.iter().any(|n| n.name == "detected_rust"));
}

#[tokio::test]
async fn targeted_mcp_refresh_removes_symbols_after_detection_stops_matching() {
    let (dir, cg) = project(true).await;
    fs::write(dir.path().join("script.custom"), PY).unwrap();
    cg.index_all().await.unwrap();
    fs::write(dir.path().join("script.custom"), "no longer source code\n").unwrap();
    assert!(!cg.sync_if_stale(&["script.custom".into()]).await.unwrap());
    assert!(cg.search("detected_name", 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn legacy_indexes_do_not_reprocess_artifacts_when_detection_is_disabled() {
    let (dir, cg) = project(false).await;
    let source = "{\"fixture\": true}\n";
    let path = dir.path().join("fixture.json");
    fs::write(&path, source).unwrap();
    let (modified_at, size) = tokensave::sync::file_stat(&path).unwrap();
    cg.db()
        .upsert_file(&tokensave::types::FileRecord {
            path: "fixture.json".into(),
            content_hash: tokensave::sync::content_hash(source),
            modified_at,
            size,
            indexed_at: tokensave::tokensave::current_timestamp(),
            node_count: 0,
            kind: tokensave::types::FileKind::Artifact,
        })
        .await
        .unwrap();
    assert!(cg
        .db()
        .get_metadata("language_detection")
        .await
        .unwrap()
        .is_none());
    let result = cg.sync().await.unwrap();
    assert_eq!(result.files_modified, 0);
    assert_eq!(result.files_added, 0);
}

#[test]
fn file_sampling_accepts_split_utf8_and_bom_shebangs() {
    let registry = LanguageRegistry::new();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("script.custom");
    let config = DetectionConfig {
        max_bytes: 128,
        ..enabled()
    };
    for character in ["é", "界", "🦀"] {
        for split in 1..character.len() {
            let source = format!("{PY}{}{character}\n", " ".repeat(128 - split - PY.len()));
            fs::write(&path, &source).unwrap();
            assert_eq!(
                detect_file(&registry, &path, config),
                detect(&registry, source.as_bytes(), config)
            );
            assert_eq!(
                detect_file(&registry, &path, config).unwrap().language,
                "Python"
            );
        }
    }
    // A genuinely incomplete codepoint at EOF still fails conservatively.
    let mut malformed = PY.as_bytes().to_vec();
    malformed.push(0xc3);
    fs::write(&path, &malformed).unwrap();
    assert!(detect_file(&registry, &path, config).is_none());
    for (interpreter, language, available) in [
        ("/usr/bin/python3", "Python", true),
        ("/bin/bash", "Bash", cfg!(feature = "lang-bash")),
    ] {
        let source = format!("\u{feff}#!{interpreter}\nf() {{ echo hi; }}\n");
        fs::write(&path, &source).unwrap();
        let result = detect_file(&registry, &path, config);
        assert_eq!(result, detect(&registry, source.as_bytes(), config));
        assert_eq!(
            result.map(|d| (d.language, d.method)),
            available.then_some((language, Method::Shebang))
        );
    }
}

#[tokio::test]
async fn detection_preserves_registered_formats_and_artifact_override_equivalence() {
    let (dir, cg) = project(false).await;
    drop(cg);
    let mut config = load_config(dir.path()).unwrap();
    config.artifact_extensions.push("custom".into());
    save_config(dir.path(), &config).unwrap();
    let cg = TokenSave::open(dir.path()).await.unwrap();
    for (path, source) in [
        ("README.md", "# Readme\n"),
        ("query.sql", "CREATE TABLE example (id INTEGER);\n"),
        ("settings.toml", "[example]\nvalue = 1\n"),
        (
            "message.proto",
            "syntax = \"proto3\";\nmessage Example { string name = 1; }\n",
        ),
        ("override.custom", PY),
        ("plain.custom", "ordinary text"),
    ] {
        fs::write(dir.path().join(path), source).unwrap();
    }
    fs::write(
        dir.path().join(".tokensave/project.json"),
        r#"{"entries":[{"path":"override.custom","language":"python"}]}"#,
    )
    .unwrap();
    cg.index_all().await.unwrap();
    drop(cg);
    for enabled_detection in [true, false, true] {
        let mut config = load_config(dir.path()).unwrap();
        config.language_detection.enabled = enabled_detection;
        save_config(dir.path(), &config).unwrap();
        let cg = TokenSave::open(dir.path()).await.unwrap();
        cg.sync().await.unwrap();
        let mut incremental: Vec<_> = cg
            .get_all_files()
            .await
            .unwrap()
            .into_iter()
            .map(|f| (f.path, f.kind))
            .collect();
        incremental.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(incremental.len(), 6);
        for (path, kind) in &incremental {
            let expected =
                if path == "plain.custom" || (path == "override.custom" && !enabled_detection) {
                    tokensave::types::FileKind::Artifact
                } else {
                    // Registered formats were already source before this feature:
                    // artifact_extensions() excludes installed extractor suffixes.
                    tokensave::types::FileKind::Code
                };
            assert_eq!(*kind, expected, "{path}");
        }
        cg.index_all().await.unwrap();
        let mut full: Vec<_> = cg
            .get_all_files()
            .await
            .unwrap()
            .into_iter()
            .map(|f| (f.path, f.kind))
            .collect();
        full.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(incremental, full);
    }
}
