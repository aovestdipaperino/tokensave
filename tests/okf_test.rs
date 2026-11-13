//! Open Knowledge Format conformance, validation and export tests (#858).

use std::path::Path;
use std::path::PathBuf;

use serde_json::json;
use serde_json::Value;
use tokensave::mcp::tools::handle_tool_call;
use tokensave::okf::bundle;
use tokensave::okf::export;
use tokensave::okf::store;
use tokensave::okf::validate;
use tokensave::okf::validate::Report;
use tokensave::okf::validate::Severity;

use crate::common::stability::Project;

/// 2026-07-02T00:00:00Z: after every fixture timestamp, before the
/// `stale_after` instants of the official sample bundles.
const NOW: i64 = 1_782_950_400;

fn official(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for part in ["tests", "fixtures", "okf", "official", name] {
        path.push(part);
    }
    path
}

fn check(root: &Path) -> Report {
    let bundle = bundle::read_default(root).expect("bundle directory is readable");
    validate::validate(&bundle, NOW)
}

fn codes(report: &Report, severity: Severity) -> Vec<(&'static str, String)> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| (d.code, d.path.clone()))
        .collect()
}

fn write_bundle(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temporary directory");
    for (path, content) in files {
        let path = bundle::join(dir.path(), path);
        std::fs::create_dir_all(path.parent().expect("file has a parent"))
            .expect("create fixture directory");
        std::fs::write(path, content).expect("write fixture file");
    }
    dir
}

#[test]
fn official_sample_bundles_are_conformant() {
    for (name, concepts, indexes, logs) in [
        ("acme_retail", 9, 7, 1),
        ("crypto_bitcoin", 9, 6, 0),
        ("ga4", 9, 5, 0),
        ("stackoverflow", 26, 6, 0),
    ] {
        let report = check(&official(name));
        assert!(report.complete, "{name}");
        assert!(report.conformant, "{name}: {:#?}", report.diagnostics);
        assert_eq!(report.errors, 0, "{name}");
        assert_eq!(
            (report.concepts, report.index_files, report.log_files),
            (concepts, indexes, logs),
            "{name}"
        );
    }
}

#[test]
fn official_acme_bundle_has_no_findings() {
    // acme_retail exercises every v0.2 family: sources, generated, bare and
    // listed verified, status, stale_after and Attested Computations.
    let report = check(&official("acme_retail"));
    assert_eq!(report.diagnostics, Vec::new());
}

#[test]
fn official_stackoverflow_findings_are_soft_guidance() {
    let report = check(&official("stackoverflow"));
    // These files write `tags` as a comma-separated string, not a list.
    let warnings = codes(&report, Severity::Warning);
    assert_eq!(warnings.len(), 8);
    assert!(warnings.iter().all(|(code, _)| *code == "OKF-W109"));
    // Positional footnotes ([^1]) instead of keyed `sources[].id` labels.
    assert!(codes(&report, Severity::Info)
        .iter()
        .all(|(code, _)| *code == "OKF-I206"));
}

#[test]
fn stale_concepts_are_reported_as_info() {
    let bundle = bundle::read_default(&official("acme_retail")).expect("readable");
    // 2027-01-01T00:00:00Z is past the bundle's 2026-12-31 stale_after.
    let report = validate::validate(&bundle, 1_798_761_600);
    assert!(report.conformant);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "OKF-I204" && d.path == "metrics/revenue.md"));
}

#[test]
fn conformance_violations_are_errors() {
    let dir = write_bundle(&[
        ("no-frontmatter.md", b"# Just a body\n"),
        ("empty-type.md", b"---\ntype: \"\"\n---\n"),
        ("numeric-type.md", b"---\ntype: 3\n---\n"),
        ("bad-yaml.md", b"---\ntype: [unclosed\n---\n"),
        ("unterminated.md", b"---\ntype: Metric\n"),
        ("list.md", b"---\n- type\n---\n"),
        ("latin1.md", b"---\ntype: Caf\xe9\n---\n"),
        ("sub/index.md", b"---\ntitle: nope\n---\n# Things\n"),
        (
            "index.md",
            b"---\nokf_version: \"0.2\"\ntitle: extra\n---\n",
        ),
        (
            "log.md",
            b"# History\n\n## 2026-05-22\n* ok\n\n## May 2026\n* bad\n",
        ),
        ("good.md", b"---\ntype: Metric\n---\n"),
    ]);
    let report = check(dir.path());
    assert!(!report.conformant);
    let mut errors = codes(&report, Severity::Error);
    errors.sort();
    assert_eq!(
        errors,
        [
            ("OKF-E001", "no-frontmatter.md".to_string()),
            ("OKF-E002", "bad-yaml.md".to_string()),
            ("OKF-E002", "list.md".to_string()),
            ("OKF-E002", "unterminated.md".to_string()),
            ("OKF-E003", "empty-type.md".to_string()),
            ("OKF-E003", "numeric-type.md".to_string()),
            ("OKF-E004", "index.md".to_string()),
            ("OKF-E004", "sub/index.md".to_string()),
            ("OKF-E005", "log.md".to_string()),
            ("OKF-E006", "latin1.md".to_string()),
        ]
    );
    let log_error = report
        .diagnostics
        .iter()
        .find(|d| d.code == "OKF-E005")
        .expect("log finding");
    assert_eq!(log_error.line, Some(6));
    assert_eq!(report.declared_version.as_deref(), Some("0.2"));
}

#[test]
fn optional_families_are_never_errors() {
    let dir = write_bundle(&[(
        "concept.md",
        b"---\n\
              type: Attested Computation\n\
              status: retired\n\
              tags: solo\n\
              generated: { at: yesterday }\n\
              verified: [{ by: human:x }, plain]\n\
              stale_after: 2026-12-31\n\
              usage_window: { from: 2026-01-01T00:00:00Z }\n\
              sources:\n\
                - { title: no resource, usage_count: -1 }\n\
                - { id: a, resource: all queries in project X }\n\
                - { id: a, resource: https://example.com, last_modified: soon }\n\
              parameters: [{ type: integer }]\n\
              executor: { receipt: [job_id] }\n\
              unknown_extension: { kept: true }\n\
              ---\n\
              Cites [^a] and [^missing]. Links to [nowhere](/nowhere.md) and [up](../../x.md).\n",
    )]);
    let report = check(dir.path());
    assert!(report.conformant, "{:#?}", report.diagnostics);
    let warnings: Vec<&str> = codes(&report, Severity::Warning)
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    for code in [
        "OKF-W101", "OKF-W102", "OKF-W103", "OKF-W104", "OKF-W105", "OKF-W106", "OKF-W108",
        "OKF-W109", "OKF-W110", "OKF-W111",
    ] {
        assert!(warnings.contains(&code), "missing {code}: {warnings:?}");
    }
    let infos: Vec<&str> = codes(&report, Severity::Info)
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    for code in ["OKF-I201", "OKF-I202", "OKF-I206"] {
        assert!(infos.contains(&code), "missing {code}: {infos:?}");
    }
}

#[test]
fn paths_differing_only_in_case_are_warned() {
    // Built in memory: case-insensitive file systems cannot hold both files.
    let file = |path: &str| bundle::BundleFile {
        path: path.to_string(),
        role: bundle::Role::Concept,
        content: bundle::parse_bytes(
            b"---
type: A
---
",
        ),
    };
    let read = bundle::Bundle {
        files: vec![file("Dir/a.md"), file("dir/A.md")],
        complete: true,
        ..bundle::Bundle::default()
    };
    let report = validate::validate(&read, NOW);
    assert!(report.conformant);
    assert_eq!(
        codes(&report, Severity::Warning),
        [("OKF-W114", "dir/A.md".to_string())]
    );
}

#[test]
fn unknown_version_is_consumed_best_effort() {
    let dir = write_bundle(&[
        ("index.md", b"---\nokf_version: \"9.0\"\n---\n# Root\n"),
        ("a.md", b"---\ntype: Thing\n---\n"),
    ]);
    let report = check(dir.path());
    assert!(report.conformant);
    assert_eq!(report.declared_version.as_deref(), Some("9.0"));
    assert!(report.diagnostics.iter().any(|d| d.code == "OKF-I207"));
}

#[test]
fn reader_limits_make_the_report_incomplete() {
    let dir = write_bundle(&[
        ("a.md", b"---\ntype: A\n---\n"),
        ("b.md", b"---\ntype: B\n---\n"),
        ("big.md", &[b'x'; 64]),
    ]);
    let limits = bundle::Limits {
        max_entries: 2,
        ..bundle::Limits::default()
    };
    let report = validate::validate(&bundle::read(dir.path(), limits).expect("readable"), NOW);
    assert!(!report.complete);
    assert!(!report.conformant);
    assert!(report.diagnostics.iter().any(|d| d.code == "OKF-E000"));

    let limits = bundle::Limits {
        max_file_bytes: 32,
        ..bundle::Limits::default()
    };
    let read = bundle::read(dir.path(), limits).expect("readable");
    assert!(read.complete);
    assert_eq!(read.skipped.len(), 1);
    assert_eq!(read.skipped[0].path, "big.md");
}

#[test]
fn hidden_directories_are_skipped() {
    let dir = write_bundle(&[
        (".git/notes.md", b"no frontmatter here"),
        ("a.md", b"---\ntype: A\n---\n"),
    ]);
    let report = check(dir.path());
    assert!(report.conformant);
    assert_eq!(report.concepts, 1);
}

#[test]
fn validating_a_file_is_refused() {
    let dir = write_bundle(&[("a.md", b"---\ntype: A\n---\n")]);
    assert!(bundle::read_default(&dir.path().join("a.md")).is_err());
}

async fn indexed_project() -> Project {
    Project::new(&[
        (
            "src/lib.rs",
            "pub mod util;\npub fn run() {\n    util::helper();\n    util::helper();\n}\n".into(),
        ),
        ("src/util.rs", "pub fn helper() {}\n".into()),
        ("tools/a b(c).rs", "pub fn tool() {}\n".into()),
    ])
    .await
}

fn options() -> export::Options {
    export::Options {
        generated_at: Some(NOW),
        ..export::Options::default()
    }
}

#[tokio::test]
async fn export_writes_a_conformant_bundle() {
    let project = indexed_project().await;
    let out = project.dir.path().join("okf-bundle");
    let summary = export::export(project.dir.path(), &out, &options())
        .await
        .expect("export succeeds");
    assert!(summary.files >= 3, "{summary:?}");
    assert!(summary.symbols >= 3, "{summary:?}");

    let report = check(&out);
    assert!(report.conformant, "{:#?}", report.diagnostics);
    // Every link and frontmatter field written by the exporter resolves.
    assert_eq!(report.diagnostics, Vec::new());
    assert_eq!(report.declared_version.as_deref(), Some("0.2"));

    let lib = std::fs::read_to_string(bundle::join(&out, "files/src/lib.rs.md")).expect("concept");
    assert!(lib.starts_with("---\ntype: \"Source File\"\n"), "{lib}");
    assert!(lib.contains("tokensave_path: \"src/lib.rs\""), "{lib}");
    assert!(lib.contains("generated: {\"by\":\"tokensave/"), "{lib}");
    assert!(lib.contains("\"at\":\"2026-07-02T00:00:00Z\"}"), "{lib}");
    assert!(lib.contains("| function | run |"), "{lib}");
    assert!(
        lib.contains("[src/util.rs](/files/src/util.rs.md) - calls ×"),
        "{lib}"
    );
    let util =
        std::fs::read_to_string(bundle::join(&out, "files/src/util.rs.md")).expect("concept");
    assert!(util.contains("# Dependents"), "{util}");

    let read = bundle::read_default(&out).expect("readable");
    let ids: Vec<&str> = read
        .files
        .iter()
        .map(bundle::BundleFile::concept_id)
        .collect();
    assert!(ids.contains(&"tokensave-export"), "{ids:?}");
    // Unportable names are rewritten; the original path is kept.
    let tool = read
        .files
        .iter()
        .find(|f| {
            f.frontmatter()
                .is_some_and(|fm| fm["tokensave_path"] == "tools/a b(c).rs")
        })
        .expect("tool file exported");
    assert_eq!(tool.path, "files/tools/a_b_c_.rs.md");
}

#[tokio::test]
async fn export_refuses_existing_output_and_leaves_no_staging() {
    let project = indexed_project().await;
    let out = project.dir.path().join("existing");
    std::fs::create_dir(&out).expect("create output");
    let err = export::export(project.dir.path(), &out, &options()).await;
    assert!(err.is_err());
    assert_eq!(std::fs::read_dir(&out).expect("readable").count(), 0);

    let limited = export::Options {
        max_files: 1,
        ..options()
    };
    let out = project.dir.path().join("too-many");
    assert!(export::export(project.dir.path(), &out, &limited)
        .await
        .is_err());
    assert!(!out.exists());
    let leftovers = std::fs::read_dir(project.dir.path())
        .expect("readable")
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".tokensave-okf-")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[tokio::test]
async fn export_truncates_long_symbol_lists() {
    let source: String = (0..5).map(|i| format!("pub fn f{i}() {{}}\n")).collect();
    let project = Project::new(&[("many.rs", source)]).await;
    let out = project.dir.path().join("bundle");
    let limited = export::Options {
        max_symbols_per_file: 2,
        ..options()
    };
    let summary = export::export(project.dir.path(), &out, &limited)
        .await
        .expect("export succeeds");
    assert_eq!(summary.truncated_files, 1);
    let text = std::fs::read_to_string(bundle::join(&out, "files/many.rs.md")).expect("concept");
    assert!(text.contains("tokensave_symbols_truncated: true"), "{text}");
    assert!(text.contains("3 more symbols are not listed."), "{text}");
    assert!(check(&out).conformant);
}

// ---------------------------------------------------------------------------
// Import, the `tokensave_knowledge` MCP tool and Graphify interoperability
// ---------------------------------------------------------------------------

fn graphify_bundle() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for part in ["tests", "fixtures", "okf", "graphify", "bundle"] {
        path.push(part);
    }
    path
}

fn named(name: &str) -> store::ImportOptions {
    store::ImportOptions {
        name: Some(name.to_string()),
        ..store::ImportOptions::default()
    }
}

async fn knowledge(project: &Project, args: Value) -> Value {
    let result = handle_tool_call(&project.cg, "tokensave_knowledge", args, None, None)
        .await
        .expect("tokensave_knowledge succeeds");
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("text content");
    serde_json::from_str(text).expect("JSON result")
}

fn result_ids(output: &Value) -> Vec<String> {
    output["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|r| {
            format!(
                "{}:{}",
                r["bundle"].as_str().unwrap_or(""),
                r["id"].as_str().unwrap_or("")
            )
        })
        .collect()
}

fn store_leftovers(project: &Project) -> Vec<String> {
    std::fs::read_dir(store::store_dir(project.dir.path()))
        .expect("store exists")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect()
}

#[test]
fn graphify_bundle_is_readable_but_not_conformant() {
    let obsidian_graph = graphify_bundle().join(".obsidian").join("graph.json");
    assert!(obsidian_graph.is_file(), "fixture is real Graphify output");
    let report = check(&graphify_bundle());
    // Graphify adds `producer: graphify` to the root index (§8 allows only
    // `okf_version`) and declares OKF 0.1.
    assert_eq!(
        codes(&report, Severity::Error),
        [("OKF-E004", "index.md".to_string())]
    );
    assert_eq!(codes(&report, Severity::Warning), Vec::new());
    assert_eq!(report.declared_version.as_deref(), Some("0.1"));
    assert!(report.diagnostics.iter().any(|d| d.code == "OKF-I207"));
    assert_eq!((report.concepts, report.index_files), (6, 3));
    // Every standard markdown link Graphify writes resolves.
    assert!(!report.diagnostics.iter().any(|d| d.code == "OKF-I201"));
}

#[tokio::test]
async fn import_copies_a_bundle_best_effort_and_strict_refuses_it() {
    let project = Project::new(&[("model.py", "class Transformer:\n    pass\n".into())]).await;
    let outcome = store::import(project.dir.path(), &graphify_bundle(), &named("graphify"))
        .expect("best-effort import succeeds");
    let record = &outcome.record;
    assert_eq!(record.name, "graphify");
    assert!(!record.conformant);
    assert_eq!(record.errors, 1);
    assert_eq!(record.concepts, 6);
    assert_eq!(record.unreadable_concepts, 0);
    // Nine markdown files and Graphify's manifest, copied byte for byte. The
    // real export's hidden .obsidian directory is deliberately skipped.
    assert_eq!(record.files, 10);
    assert!(!outcome.replaced);
    let stored = store::store_dir(project.dir.path()).join("graphify");
    for rel in ["concepts/Transformer.md", ".graphify_okf_manifest.json"] {
        assert_eq!(
            std::fs::read(bundle::join(&stored.join(store::BUNDLE_DIR), rel)).expect("copied"),
            std::fs::read(bundle::join(&graphify_bundle(), rel)).expect("source"),
            "{rel}"
        );
    }

    let strict = store::ImportOptions {
        strict: true,
        ..named("strict")
    };
    assert!(store::import(project.dir.path(), &graphify_bundle(), &strict).is_err());
    assert!(!store::store_dir(project.dir.path()).join("strict").exists());
    assert_eq!(store_leftovers(&project), Vec::<String>::new());

    let listed = store::list(project.dir.path()).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].record, outcome.record);
}

#[tokio::test]
async fn import_names_replace_and_remove() {
    let project = Project::new(&[("a.rs", "pub fn a() {}\n".into())]).await;
    let root = project.dir.path();
    // The default name is the directory name.
    let first =
        store::import(root, &official("ga4"), &store::ImportOptions::default()).expect("import");
    assert_eq!(first.record.name, "ga4");
    assert!(first.record.conformant);

    assert!(store::import(root, &official("crypto_bitcoin"), &named("ga4")).is_err());
    assert!(store::import(root, &official("ga4"), &named("../escape")).is_err());
    assert!(store::import(root, &official("ga4"), &named("con")).is_err());

    let replace = store::ImportOptions {
        replace: true,
        ..named("ga4")
    };
    let second = store::import(root, &official("crypto_bitcoin"), &replace).expect("replace");
    assert!(second.replaced);
    let listed = store::list(root).expect("list");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].record.source.contains("crypto_bitcoin"));
    assert_eq!(store_leftovers(&project), Vec::<String>::new());

    assert!(store::remove(root, "ga4").expect("remove"));
    assert!(!store::remove(root, "ga4").expect("second remove"));
    assert!(store::list(root).expect("list").is_empty());
    assert_eq!(store_leftovers(&project), Vec::<String>::new());
}

#[test]
fn import_requires_an_initialized_project() {
    let dir = tempfile::tempdir().expect("temporary directory");
    assert!(store::import(dir.path(), &official("ga4"), &named("ga4")).is_err());
}

#[tokio::test]
async fn knowledge_tool_lists_searches_and_reads_imported_concepts() {
    let project = Project::new(&[("model.py", "class Transformer:\n    pass\n".into())]).await;
    let empty = knowledge(&project, json!({})).await;
    assert_eq!(empty["bundles"], json!([]));

    store::import(project.dir.path(), &graphify_bundle(), &named("graphify")).expect("import");
    store::import(
        project.dir.path(),
        &official("acme_retail"),
        &store::ImportOptions::default(),
    )
    .expect("import");

    let listing = knowledge(&project, json!({})).await;
    let names: Vec<&str> = listing["bundles"]
        .as_array()
        .expect("bundles")
        .iter()
        .filter_map(|b| b["name"].as_str())
        .collect();
    assert_eq!(names, ["acme_retail", "graphify"]);

    let found = knowledge(&project, json!({"query": "gross margin", "type": "metric"})).await;
    let ids = result_ids(&found);
    assert_eq!(
        ids.first().map(String::as_str),
        Some("acme_retail:metrics/gross-margin"),
        "{ids:?}"
    );

    // Graphify concepts name their source file, which is indexed here.
    let attention = knowledge(
        &project,
        json!({"query": "attention", "bundle": "graphify"}),
    )
    .await;
    let ids = result_ids(&attention);
    assert!(
        ids.contains(&"graphify:concepts/attention_mechanism".to_string()),
        "{ids:?}"
    );
    let multi = attention["results"]
        .as_array()
        .expect("results")
        .iter()
        .find(|r| r["id"] == "concepts/MultiHeadAttention")
        .expect("MultiHeadAttention found");
    assert_eq!(
        multi["code_files"],
        json!([{"path": "model.py", "indexed": true}])
    );
    let paper = attention["results"]
        .as_array()
        .expect("results")
        .iter()
        .find(|r| r["id"] == "concepts/attention_mechanism")
        .expect("attention mechanism found");
    assert_eq!(
        paper["code_files"],
        json!([{"path": "paper.md", "indexed": false}])
    );

    let by_file = knowledge(&project, json!({"file": "model.py"})).await;
    assert_eq!(result_ids(&by_file).len(), 3);

    let read = knowledge(&project, json!({"concept": "concepts/Transformer.md"})).await;
    assert_eq!(read["bundle"], "graphify");
    assert_eq!(read["type"], "Graphify Code Concept");
    assert_eq!(read["frontmatter"]["graphify_id"], "n_transformer");
    assert!(read["body"]
        .as_str()
        .expect("body")
        .contains("## Connections"));
    assert!(read["links"]
        .as_array()
        .expect("links")
        .contains(&json!({"id": "concepts/LayerNorm", "exists": true})));
    let backlinks = read["backlinks"].as_array().expect("backlinks");
    assert!(
        backlinks.contains(&json!("communities/Model_Architecture")),
        "{backlinks:?}"
    );
    assert!(
        backlinks.contains(&json!("concepts/MultiHeadAttention")),
        "{backlinks:?}"
    );

    let missing = knowledge(&project, json!({"concept": "nope"})).await;
    assert!(missing["error"].as_str().is_some());
    assert!(handle_tool_call(
        &project.cg,
        "tokensave_knowledge",
        json!({"query": "x", "bundle": "unknown"}),
        None,
        None
    )
    .await
    .is_err());

    // Removing a bundle is seen by the next call.
    store::remove(project.dir.path(), "graphify").expect("remove");
    let after = knowledge(&project, json!({"query": "attention"})).await;
    assert!(result_ids(&after)
        .iter()
        .all(|id| !id.starts_with("graphify:")));
}

#[tokio::test]
async fn exported_bundles_round_trip_through_import() {
    let project = indexed_project().await;
    let out = project.dir.path().join("okf-bundle");
    export::export(project.dir.path(), &out, &options())
        .await
        .expect("export succeeds");
    let outcome = store::import(project.dir.path(), &out, &named("self")).expect("import");
    assert!(outcome.record.conformant);
    assert_eq!(outcome.report.diagnostics, Vec::new());
    assert_eq!(outcome.record.declared_version.as_deref(), Some("0.2"));

    // A Source File concept maps back to its indexed file.
    let by_file = knowledge(&project, json!({"file": "src/util.rs"})).await;
    assert_eq!(result_ids(&by_file), ["self:files/src/util.rs"]);
    assert_eq!(
        by_file["results"][0]["code_files"],
        json!([{"path": "src/util.rs", "indexed": true}])
    );
    // Symbol names in the concept body are searchable.
    let helper = knowledge(&project, json!({"query": "helper"})).await;
    assert_eq!(
        result_ids(&helper).first().map(String::as_str),
        Some("self:files/src/util.rs")
    );
    let read = knowledge(
        &project,
        json!({"concept": "files/src/lib.rs", "bundle": "self"}),
    )
    .await;
    assert!(read["links"]
        .as_array()
        .expect("links")
        .contains(&json!({"id": "files/src/util.rs", "exists": true})));
    // The exported original path survives the round trip, including
    // characters the bundle file names cannot hold.
    let tool = knowledge(&project, json!({"file": "tools/a b(c).rs"})).await;
    assert_eq!(result_ids(&tool), ["self:files/tools/a_b_c_.rs"]);
}
