//! Open Knowledge Format conformance, validation and export tests (#858).

use std::path::Path;
use std::path::PathBuf;

use tokensave::okf::bundle;
use tokensave::okf::export;
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
