//! Open Knowledge Format conformance, validation, export and import tests
//! (#858, #1060).

use std::path::Path;
use std::path::PathBuf;

use serde_json::json;
use serde_json::Value;
use tokensave::mcp::tools::handle_tool_call;
use tokensave::okf::bundle;
use tokensave::okf::consumer::ReadMode;
use tokensave::okf::export;
use tokensave::okf::store;
use tokensave::okf::validate;
use tokensave::okf::validate::Report;
use tokensave::okf::validate::Severity;

use crate::common::stability::Project;

/// 2026-07-02T00:00:00Z: after every fixture timestamp, before the
/// `stale_after` instants of the official sample bundles.
const NOW: i64 = 1_782_950_400;

/// The OKF versions of the vendored official sample bundles.
const VERSIONS: [&str; 2] = ["v0.2", "v0.3"];

fn official(version: &str, name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for part in ["tests", "fixtures", "okf", "official", version, name] {
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

fn code_list(report: &Report, severity: Severity) -> Vec<&'static str> {
    codes(report, severity)
        .into_iter()
        .map(|(code, _)| code)
        .collect()
}

fn write_bundle(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temporary directory");
    write_into(dir.path(), files);
    dir
}

fn write_into(root: &Path, files: &[(&str, &[u8])]) {
    for (path, content) in files {
        let path = bundle::join(root, path);
        std::fs::create_dir_all(path.parent().expect("file has a parent"))
            .expect("create fixture directory");
        std::fs::write(path, content).expect("write fixture file");
    }
}

/// Copies a fixture bundle into a new temporary directory.
fn copy_bundle(source: &Path) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temporary directory");
    let read = bundle::read_default(source).expect("readable");
    for rel in &read.entries {
        let target = bundle::join(dir.path(), rel);
        if rel.ends_with('/') {
            std::fs::create_dir_all(target).expect("create directory");
        } else {
            std::fs::create_dir_all(target.parent().expect("parent")).expect("create parent");
            std::fs::copy(bundle::join(source, rel), target).expect("copy file");
        }
    }
    dir
}

#[test]
fn official_sample_bundles_are_conformant_in_every_version() {
    for version in VERSIONS {
        for (name, concepts, indexes, logs) in [
            ("acme_retail", 9, 7, 1),
            ("crypto_bitcoin", 9, 6, 0),
            ("ga4", 9, 5, 0),
            ("stackoverflow", 26, 6, 0),
        ] {
            let report = check(&official(version, name));
            assert!(report.complete, "{version}/{name}");
            assert!(
                report.conformant,
                "{version}/{name}: {:#?}",
                report.diagnostics
            );
            assert_eq!(report.errors, 0, "{version}/{name}");
            assert_eq!(report.non_concepts, 0, "{version}/{name}");
            assert_eq!(
                (report.concepts, report.index_files, report.log_files),
                (concepts, indexes, logs),
                "{version}/{name}"
            );
        }
    }
}

#[test]
fn official_v0_3_bundles_have_no_findings() {
    // acme_retail exercises every v0.3 family: bundle metadata, sources,
    // generated, bare and listed verified, status, superseded_by,
    // stale_after, extension keys and Attested Computations.
    for name in ["acme_retail", "crypto_bitcoin", "ga4", "stackoverflow"] {
        let report = check(&official("v0.3", name));
        assert_eq!(report.diagnostics, Vec::new(), "{name}");
        assert_eq!(report.read_mode, ReadMode::Current, "{name}");
        assert_eq!(report.declared_version.as_deref(), Some("0.3"), "{name}");
    }
    let report = check(&official("v0.3", "acme_retail"));
    assert_eq!(report.metadata.title.as_deref(), Some("Acme Retail"));
    assert!(report
        .metadata
        .description
        .as_deref()
        .is_some_and(|d| d.contains("gross-margin")));
}

#[test]
fn v0_2_bundles_are_read_with_the_fallbacks() {
    let report = check(&official("v0.2", "acme_retail"));
    assert!(report.conformant, "{:#?}", report.diagnostics);
    assert_eq!(report.read_mode, ReadMode::Legacy);
    assert_eq!(report.declared_version, None);
    // Its log.md has frontmatter, which OKF 0.3 forbids; tolerated here.
    assert_eq!(
        codes(&report, Severity::Warning),
        [("OKF-W116", "log.md".to_string())]
    );
    let infos = code_list(&report, Severity::Info);
    assert!(infos.contains(&"OKF-I210"), "{infos:?}");
    // v0.2 wrote bundle-root path fields without a leading `/`.
    assert!(infos.contains(&"OKF-I211"), "{infos:?}");
    assert!(!infos.contains(&"OKF-I201"), "{infos:?}");
    // The fallback finds every computation, so none is unattestable.
    assert!(!infos.contains(&"OKF-I209"), "{infos:?}");
}

#[test]
fn v0_2_bundle_declared_as_v0_3_loses_the_fallbacks() {
    // Relabelling a v0.2 bundle without migrating it is the breaking change
    // of OKF v0.3 §13.1: bare paths now resolve from the file's directory.
    let dir = copy_bundle(&official("v0.2", "acme_retail"));
    let index = bundle::join(dir.path(), "index.md");
    let text = std::fs::read_to_string(&index).expect("index");
    std::fs::write(&index, format!("---\nokf_version: \"0.3\"\n---\n\n{text}")).expect("write");
    let report = check(dir.path());
    assert_eq!(report.read_mode, ReadMode::Current);
    assert!(!report.conformant);
    assert_eq!(
        codes(&report, Severity::Error),
        [("OKF-E007", "log.md".to_string())]
    );
    let warnings = code_list(&report, Severity::Warning);
    assert!(warnings.iter().all(|c| *c == "OKF-W123"), "{warnings:?}");
    assert_eq!(warnings.len(), 12, "{:#?}", report.diagnostics);
    let w123 = report
        .diagnostics
        .iter()
        .find(|d| d.code == "OKF-W123")
        .expect("W123");
    assert!(w123.message.contains("Write \"/"), "{}", w123.message);
    // Without the fallback the attester files cannot be found, so the
    // computations are not attestable.
    let i209: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "OKF-I209")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(i209.len(), 2, "{i209:?}");
    assert!(i209
        .iter()
        .all(|m| m.contains("attester file not in the bundle")));
}

#[test]
fn official_stackoverflow_v0_2_findings_are_soft_guidance() {
    let report = check(&official("v0.2", "stackoverflow"));
    // These files write `tags` as a comma-separated string, not a list.
    let warnings = codes(&report, Severity::Warning);
    assert_eq!(warnings.len(), 8);
    assert!(warnings.iter().all(|(code, _)| *code == "OKF-W109"));
    // Positional footnotes ([^1]) instead of keyed `sources[].id` labels.
    assert!(code_list(&report, Severity::Info)
        .iter()
        .all(|code| matches!(*code, "OKF-I206" | "OKF-I210")));
}

#[test]
fn stale_concepts_are_reported_as_info() {
    let bundle = bundle::read_default(&official("v0.3", "acme_retail")).expect("readable");
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
        ("bad-yaml.md", b"---\ntype: [unclosed\n---\n"),
        ("unterminated.md", b"---\ntype: Metric\n"),
        ("list.md", b"---\n- type\n---\n"),
        ("duplicate.md", b"---\ntype: A\ntype: B\n---\n"),
        ("latin1.md", b"---\ntype: Caf\xe9\n---\n"),
        ("sub/index.md", b"---\ntitle: nope\n---\n# Things\n"),
        (
            "index.md",
            b"---\nokf_version: \"0.3\"\ntitle: Bundle\nx-acme-owner: team:x\n---\n",
        ),
        (
            "log.md",
            b"---\ntype: Log\n---\n# History\n\n## 2026-05-22\n* ok\n\n## May 2026\n* bad\n",
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
            ("OKF-E002", "bad-yaml.md".to_string()),
            ("OKF-E002", "duplicate.md".to_string()),
            ("OKF-E002", "list.md".to_string()),
            ("OKF-E002", "unterminated.md".to_string()),
            ("OKF-E004", "sub/index.md".to_string()),
            ("OKF-E005", "log.md".to_string()),
            ("OKF-E006", "latin1.md".to_string()),
            ("OKF-E007", "log.md".to_string()),
        ]
    );
    let log_error = report
        .diagnostics
        .iter()
        .find(|d| d.code == "OKF-E005")
        .expect("log finding");
    assert_eq!(log_error.line, Some(9));
    let duplicate = report
        .diagnostics
        .iter()
        .find(|d| d.path == "duplicate.md")
        .expect("duplicate finding");
    assert!(
        duplicate.message.contains("duplicated key"),
        "{}",
        duplicate.message
    );
    // The root index may carry bundle metadata and extension keys (§8.1).
    assert_eq!(report.declared_version.as_deref(), Some("0.3"));
    assert_eq!(report.metadata.title.as_deref(), Some("Bundle"));
    assert_eq!(report.metadata.extensions["x-acme-owner"], "team:x");
}

#[test]
fn markdown_files_without_type_are_not_concepts() {
    // §3.2: README.md, AGENTS.md and similar files may live in a bundle.
    let dir = write_bundle(&[
        ("index.md", b"---\nokf_version: \"0.3\"\n---\n"),
        ("README.md", b"# About this bundle\n"),
        ("notes.md", b"---\ntitle: Notes\n---\nNo type.\n"),
        ("empty-type.md", b"---\ntype: \"\"\n---\n"),
        ("numeric-type.md", b"---\ntype: 3\n---\n"),
        ("metric.md", b"---\ntype: Metric\n---\n"),
    ]);
    let report = check(dir.path());
    assert!(report.conformant, "{:#?}", report.diagnostics);
    assert_eq!((report.concepts, report.non_concepts), (1, 4));
    let mut warnings = codes(&report, Severity::Warning);
    warnings.sort();
    assert_eq!(
        warnings,
        [
            ("OKF-W117", "README.md".to_string()),
            ("OKF-W118", "empty-type.md".to_string()),
            ("OKF-W118", "notes.md".to_string()),
            ("OKF-W118", "numeric-type.md".to_string()),
        ]
    );
}

#[test]
fn optional_families_are_never_errors() {
    let dir = write_bundle(&[(
        "concept.md",
        b"---\n\
              type: Attested Computation\n\
              title: [not, a, string]\n\
              status: retired\n\
              tags: solo\n\
              generated: { at: yesterday }\n\
              verified: [{ by: human:x }, plain, { by: Jane Doe, at: 2026-01-01 }]\n\
              stale_after: 2026-12-31\n\
              usage_window: { from: 2026-01-01T00:00:00Z }\n\
              sources:\n\
                - { title: no resource, usage_count: -1 }\n\
                - { id: a, resource: all queries in project X }\n\
                - { id: a, resource: https://example.com, last_modified: soon }\n\
              parameters: [{ type: integer }]\n\
              executor: { receipt: [job_id] }\n\
              custom: !acme/thing x\n\
              unknown_extension: { kept: true }\n\
              ---\n\
              Cites [^a] and [^missing]. Links to [nowhere](/nowhere.md) and [up](../../x.md).\n",
    )]);
    let report = check(dir.path());
    assert!(report.conformant, "{:#?}", report.diagnostics);
    let warnings = code_list(&report, Severity::Warning);
    for code in [
        "OKF-W101", "OKF-W102", "OKF-W103", "OKF-W104", "OKF-W105", "OKF-W106", "OKF-W108",
        "OKF-W109", "OKF-W110", "OKF-W111", "OKF-W119", "OKF-W124",
    ] {
        assert!(warnings.contains(&code), "missing {code}: {warnings:?}");
    }
    let infos = code_list(&report, Severity::Info);
    for code in ["OKF-I201", "OKF-I202", "OKF-I206", "OKF-I209"] {
        assert!(infos.contains(&code), "missing {code}: {infos:?}");
    }
    // A date-only `stale_after` is valid (§5); only `yesterday`, `soon` and
    // the missing `usage_window.to` are malformed timestamps.
    let w102 = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "OKF-W102")
        .count();
    assert_eq!(w102, 3, "{:#?}", report.diagnostics);
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
    assert_eq!(report.read_mode, ReadMode::BestEffort);
    assert!(report.diagnostics.iter().any(|d| d.code == "OKF-I207"));

    let dir = write_bundle(&[("index.md", b"---\nokf_version: 0.3\n---\n")]);
    let report = check(dir.path());
    assert_eq!(report.read_mode, ReadMode::Current);
    assert_eq!(
        code_list(&report, Severity::Warning),
        ["OKF-W112"],
        "an unquoted version is a YAML number"
    );
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
fn hidden_files_and_directories_are_not_part_of_the_bundle() {
    let dir = write_bundle(&[
        (".git/notes.md", b"no frontmatter here"),
        (".draft.md", b"no frontmatter here either"),
        (".manifest.json", b"{}"),
        ("a.md", b"---\ntype: A\n---\n"),
    ]);
    let read = bundle::read_default(dir.path()).expect("readable");
    assert_eq!(
        read.entries.iter().collect::<Vec<_>>(),
        [&"a.md".to_string()]
    );
    let report = validate::validate(&read, NOW);
    assert!(report.conformant);
    assert_eq!((report.concepts, report.non_concepts), (1, 0));
    assert_eq!(report.warnings, 0, "{:#?}", report.diagnostics);
}

#[test]
fn validating_a_file_is_refused() {
    let dir = write_bundle(&[("a.md", b"---\ntype: A\n---\n")]);
    assert!(bundle::read_default(&dir.path().join("a.md")).is_err());
}

// ---------------------------------------------------------------------------
// A v0.3 bundle exercising paths, trust, lifecycle and extensions
// ---------------------------------------------------------------------------

/// A bundle that uses every v0.3 capability tokensave reads. The expected
/// findings of each file are stated next to it.
const V03_BUNDLE: &[(&str, &[u8])] = &[
    (
        "index.md",
        b"---\n\
          okf_version: \"0.3\"\n\
          title: Finance knowledge\n\
          description: Revenue metrics and computations. Use it for revenue questions.\n\
          license: /LICENSE.txt\n\
          x-acme-steward: { team: \"team:fpa\", contacts: [a, b] }\n\
          ---\n\n# Metrics\n\n* [Revenue](metrics/revenue.md) - Recognized revenue.\n",
    ),
    ("LICENSE.txt", b"CC-BY-4.0\n"),
    (
        "log.md",
        b"# Bundle history\n\n## 2026-07-01\n\n* **Creation**: Wrote the bundle.\n",
    ),
    // Not concepts: skipped with OKF-W117 and OKF-W118.
    ("README.md", b"# How to edit this bundle\n"),
    ("notes.md", b"---\ntitle: Scratch notes\n---\n"),
    ("attesters/check.py", b"def attest(receipt):\n    return {'ok': True}\n"),
    (
        "policies/recognition.md",
        b"---\ntype: Policy\ntitle: Revenue recognition\n---\nWhen revenue counts.\n",
    ),
    // Human-reviewed after the last change; a relative and a root path
    // field; nested extensions at several levels; broken, external,
    // escaping and asset links.
    (
        "metrics/revenue.md",
        b"---\n\
          type: Metric\n\
          title: Revenue\n\
          description: Recognized revenue for a fiscal year.\n\
          tags: finance, revenue\n\
          generated: { by: reference_agent/gemini-2.5-pro, at: \"2026-06-20T14:00:00Z\", x-acme-run: 7 }\n\
          verified: { by: \"human:ana\", at: \"2026-06-25T09:00:00Z\" }\n\
          stale_after: \"2999-01-01\"\n\
          sources:\n  - id: policy\n    resource: ../policies/recognition.md\n    \
          author: \"team:fpa\"\n    \
          x-acme-confidence: { level: high, reasons: [audited, current] }\n  \
          - { id: dash, resource: /policies/recognition.md, usage_count: 5000 }\n\
          x-acme-owner: { team: \"team:fpa\", oncall: [\"human:ana\"] }\n\
          ---\n\n\
          # Definition\n\n\
          Computed by [the computation](../computations/revenue.md).[^policy]\n\
          See [orders](../tables/orders.md), [the attester](/attesters/check.py),\n\
          [the docs](https://example.com/revenue.md) and [outside](../../outside.md).\n\n\
          [^policy]: Revenue recognition policy\n",
    ),
    // Rewritten by an agent after the human review: the review is history
    // (OKF-I208) and only the later process check counts.
    (
        "metrics/rewritten.md",
        b"---\n\
          type: Metric\n\
          title: Net revenue\n\
          generated: { by: reference_agent/gemini-2.5-pro, at: \"2026-06-30T00:00:00Z\" }\n\
          verified:\n\
            - { by: \"human:ana\", at: \"2026-06-25T00:00:00Z\" }\n\
            - { by: \"process:nightly\", at: \"2026-07-01T00:00:00Z\" }\n\
            - { by: \"human:bob\", at: \"2026-06-29T00:00:00Z\" }\n\
          stale_after: \"2000-01-01T00:00:00Z\"\n\
          ---\n",
    ),
    // A deprecated chain: older -> old -> revenue.
    (
        "metrics/old.md",
        b"---\ntype: Metric\ntitle: Legacy revenue\nstatus: deprecated\nsuperseded_by: ./revenue.md\n---\n",
    ),
    (
        "metrics/older.md",
        b"---\ntype: Metric\ntitle: Ancient revenue\nstatus: deprecated\nsuperseded_by: /metrics/old.md\n---\n",
    ),
    // OKF-W122 for both.
    (
        "metrics/loop-a.md",
        b"---\ntype: Metric\ntitle: Loop A\nstatus: deprecated\nsuperseded_by: loop-b.md\n---\n",
    ),
    (
        "metrics/loop-b.md",
        b"---\ntype: Metric\ntitle: Loop B\nstatus: deprecated\nsuperseded_by: loop-a.md\n---\n",
    ),
    // OKF-W121: names nothing. OKF-W120: superseded_by without deprecated.
    (
        "metrics/dangling.md",
        b"---\ntype: Metric\ntitle: Dangling\nstatus: deprecated\nsuperseded_by: /metrics/nowhere.md\n---\n",
    ),
    (
        "metrics/draft.md",
        b"---\ntype: Metric\ntitle: Draft margin\nstatus: draft\nsuperseded_by: ./revenue.md\n---\n",
    ),
    // OKF-W123: a v0.2 bundle-root path in a v0.3 bundle; also one
    // `sources` mapping without the list dash.
    (
        "metrics/bare.md",
        b"---\ntype: Metric\ntitle: Bare path\nsources: { resource: policies/recognition.md }\n---\n",
    ),
    // Attestable: runtime, attester and a fenced block in a level-2
    // Computation section.
    (
        "computations/revenue.md",
        b"---\n\
          type: Attested Computation\n\
          title: Revenue for fiscal year\n\
          runtime: bigquery\n\
          parameters: { name: year, type: integer, required: true }\n\
          attester: { resource: /attesters/check.py }\n\
          ---\n\n\
          ## Computation\n\n\
          ```sql\nSELECT SUM(amount) FROM revenue WHERE fiscal_year = @year\n```\n",
    ),
    // Not attestable (OKF-I209): no attester.
    (
        "computations/no-attester.md",
        b"---\ntype: Attested Computation\ntitle: Unchecked\nruntime: bigquery\n---\n\n# Computation\n\n```sql\nSELECT 1\n```\n",
    ),
];

#[test]
fn v0_3_semantics_are_validated() {
    let dir = write_bundle(V03_BUNDLE);
    let report = check(dir.path());
    assert!(report.conformant, "{:#?}", report.diagnostics);
    assert_eq!(report.read_mode, ReadMode::Current);
    assert_eq!((report.concepts, report.non_concepts), (12, 2));
    assert_eq!(report.metadata.license.as_deref(), Some("/LICENSE.txt"));
    assert_eq!(
        report.metadata.extensions["x-acme-steward"],
        json!({"team": "team:fpa", "contacts": ["a", "b"]})
    );
    let mut warnings = codes(&report, Severity::Warning);
    warnings.sort();
    assert_eq!(
        warnings,
        [
            ("OKF-W109", "metrics/revenue.md".to_string()),
            ("OKF-W117", "README.md".to_string()),
            ("OKF-W118", "notes.md".to_string()),
            ("OKF-W120", "metrics/draft.md".to_string()),
            ("OKF-W121", "metrics/dangling.md".to_string()),
            ("OKF-W122", "metrics/loop-a.md".to_string()),
            ("OKF-W122", "metrics/loop-b.md".to_string()),
            ("OKF-W123", "metrics/bare.md".to_string()),
        ],
        "{:#?}",
        report.diagnostics
    );
    let mut infos = codes(&report, Severity::Info);
    infos.sort();
    assert_eq!(
        infos,
        [
            ("OKF-I201", "metrics/revenue.md".to_string()),
            ("OKF-I202", "metrics/revenue.md".to_string()),
            ("OKF-I204", "metrics/rewritten.md".to_string()),
            ("OKF-I208", "metrics/rewritten.md".to_string()),
            ("OKF-I209", "computations/no-attester.md".to_string()),
        ],
        "{:#?}",
        report.diagnostics
    );
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

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
async fn export_writes_a_conformant_v0_3_bundle() {
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
    assert_eq!(report.declared_version.as_deref(), Some("0.3"));
    assert_eq!(report.read_mode, ReadMode::Current);
    assert_eq!(
        report.metadata.title.as_deref(),
        Some("TokenSave code index")
    );
    assert!(report.metadata.description.is_some());

    let lib = std::fs::read_to_string(bundle::join(&out, "files/src/lib.rs.md")).expect("concept");
    assert!(lib.starts_with("---\ntype: \"Source File\"\n"), "{lib}");
    assert!(lib.contains("x-tokensave-path: \"src/lib.rs\""), "{lib}");
    assert!(!lib.contains("tokensave_"), "{lib}");
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
                .is_some_and(|fm| fm["x-tokensave-path"] == "tools/a b(c).rs")
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
    assert!(
        text.contains("x-tokensave-symbols-truncated: true"),
        "{text}"
    );
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
fn graphify_bundle_is_conformant_with_legacy_notes() {
    let obsidian_graph = graphify_bundle().join(".obsidian").join("graph.json");
    assert!(obsidian_graph.is_file(), "fixture is real Graphify output");
    let report = check(&graphify_bundle());
    // Graphify adds `producer: graphify` to the root index, an extension
    // key that OKF 0.3 §8.1 permits, and declares OKF 0.1.
    assert!(report.conformant, "{:#?}", report.diagnostics);
    assert_eq!(report.warnings, 0, "{:#?}", report.diagnostics);
    assert_eq!(report.declared_version.as_deref(), Some("0.1"));
    assert_eq!(report.read_mode, ReadMode::Legacy);
    assert_eq!(report.metadata.extensions["producer"], "graphify");
    assert!(report.diagnostics.iter().any(|d| d.code == "OKF-I207"));
    assert_eq!((report.concepts, report.index_files), (6, 3));
    // Every standard markdown link Graphify writes resolves.
    assert!(!report.diagnostics.iter().any(|d| d.code == "OKF-I201"));
}

#[tokio::test]
async fn import_copies_a_bundle_and_strict_refuses_a_non_conformant_one() {
    let project = Project::new(&[("model.py", "class Transformer:\n    pass\n".into())]).await;
    let strict = |name: &str| store::ImportOptions {
        strict: true,
        ..named(name)
    };
    let outcome = store::import(project.dir.path(), &graphify_bundle(), &strict("graphify"))
        .expect("a conformant bundle imports under --strict");
    let record = &outcome.record;
    assert_eq!(record.name, "graphify");
    assert!(record.conformant);
    assert_eq!(record.errors, 0);
    assert_eq!(record.declared_version.as_deref(), Some("0.1"));
    assert_eq!(record.concepts, 6);
    assert_eq!(record.unreadable_concepts, 0);
    // Nine markdown files, copied byte for byte. Hidden files and
    // directories (Graphify's manifest and .obsidian/) are not part of the
    // bundle (§3.2) and are not copied.
    assert_eq!(record.files, 9);
    assert!(!outcome.replaced);
    let stored = store::store_dir(project.dir.path())
        .join("graphify")
        .join(store::BUNDLE_DIR);
    assert_eq!(
        std::fs::read(bundle::join(&stored, "concepts/Transformer.md")).expect("copied"),
        std::fs::read(bundle::join(&graphify_bundle(), "concepts/Transformer.md")).expect("source"),
    );
    assert!(!bundle::join(&stored, ".graphify_okf_manifest.json").exists());

    let broken = write_bundle(&[
        ("a.md", b"---\ntype: A\n---\n"),
        (
            "sub/index.md",
            b"---\ntitle: frontmatter is not allowed here\n---\n",
        ),
    ]);
    assert!(store::import(project.dir.path(), broken.path(), &strict("strict")).is_err());
    assert!(!store::store_dir(project.dir.path()).join("strict").exists());
    let best_effort =
        store::import(project.dir.path(), broken.path(), &named("lenient")).expect("best effort");
    assert!(!best_effort.record.conformant);
    assert_eq!(best_effort.record.errors, 1);
    assert_eq!(store_leftovers(&project), Vec::<String>::new());

    let listed = store::list(project.dir.path()).expect("list");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].record, outcome.record);
}

#[tokio::test]
async fn import_names_replace_and_remove() {
    let project = Project::new(&[("a.rs", "pub fn a() {}\n".into())]).await;
    let root = project.dir.path();
    // The default name is the directory name.
    let first = store::import(
        root,
        &official("v0.3", "ga4"),
        &store::ImportOptions::default(),
    )
    .expect("import");
    assert_eq!(first.record.name, "ga4");
    assert!(first.record.conformant);
    assert_eq!(first.record.declared_version.as_deref(), Some("0.3"));

    assert!(store::import(root, &official("v0.3", "crypto_bitcoin"), &named("ga4")).is_err());
    assert!(store::import(root, &official("v0.3", "ga4"), &named("../escape")).is_err());
    assert!(store::import(root, &official("v0.3", "ga4"), &named("con")).is_err());

    let replace = store::ImportOptions {
        replace: true,
        ..named("ga4")
    };
    let second =
        store::import(root, &official("v0.3", "crypto_bitcoin"), &replace).expect("replace");
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
    assert!(store::import(dir.path(), &official("v0.3", "ga4"), &named("ga4")).is_err());
}

#[tokio::test]
async fn knowledge_tool_lists_searches_and_reads_imported_concepts() {
    let project = Project::new(&[("model.py", "class Transformer:\n    pass\n".into())]).await;
    let empty = knowledge(&project, json!({})).await;
    assert_eq!(empty["bundles"], json!([]));

    store::import(project.dir.path(), &graphify_bundle(), &named("graphify")).expect("import");
    store::import(
        project.dir.path(),
        &official("v0.3", "acme_retail"),
        &store::ImportOptions::default(),
    )
    .expect("import");

    let listing = knowledge(&project, json!({})).await;
    let bundles = listing["bundles"].as_array().expect("bundles");
    let names: Vec<&str> = bundles.iter().filter_map(|b| b["name"].as_str()).collect();
    assert_eq!(names, ["acme_retail", "graphify"]);
    assert_eq!(bundles[0]["title"], "Acme Retail");
    assert_eq!(bundles[0]["declared_okf_version"], "0.3");
    assert_eq!(bundles[0]["read_mode"], "current");
    assert_eq!(bundles[1]["read_mode"], "legacy");

    let found = knowledge(&project, json!({"query": "gross margin", "type": "metric"})).await;
    let ids = result_ids(&found);
    assert_eq!(
        ids.first().map(String::as_str),
        Some("acme_retail:metrics/gross-margin"),
        "{ids:?}"
    );
    assert_eq!(found["results"][0]["trust_tier"], "human-reviewed");
    assert_eq!(found["results"][0]["current"], true);

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

    let related = knowledge(
        &project,
        json!({"related": "concepts/Transformer", "bundle": "graphify", "depth": 2}),
    )
    .await;
    assert_eq!(related["depth"], 2);
    let related_ids = result_ids(&related);
    assert!(
        related_ids.contains(&"graphify:concepts/LayerNorm".to_string()),
        "{related_ids:?}"
    );
    assert!(
        related_ids.contains(&"graphify:communities/Model_Architecture".to_string()),
        "{related_ids:?}"
    );
    assert!(related["results"]
        .as_array()
        .expect("related results")
        .iter()
        .any(|result| result["distance"] == 2));

    // The official deprecated metric leads to its replacement (§5.4).
    let legacy = knowledge(
        &project,
        json!({"query": "legacy pre-FY2026", "bundle": "acme_retail"}),
    )
    .await;
    let first = &legacy["results"][0];
    assert_eq!(first["id"], "metrics/gross-margin-legacy");
    assert_eq!(first["status"], "deprecated");
    assert_eq!(first["current"], false);
    assert_eq!(first["superseded_by"], "/metrics/gross-margin.md");
    assert_eq!(first["replacement"], "metrics/gross-margin");
    let current = knowledge(
        &project,
        json!({"query": "legacy pre-FY2026", "bundle": "acme_retail", "current_only": true}),
    )
    .await;
    let first = &current["results"][0];
    assert_eq!(first["id"], "metrics/gross-margin");
    assert_eq!(first["replaces"], "metrics/gross-margin-legacy");
    assert!(result_ids(&current)
        .iter()
        .all(|id| id != "acme_retail:metrics/gross-margin-legacy"));
    // Path fields are graph edges (§6.4): the policy is a source of the
    // gross-margin metric.
    let policy = knowledge(
        &project,
        json!({"concept": "policies/margin-standard", "bundle": "acme_retail"}),
    )
    .await;
    assert!(policy["backlinks"]
        .as_array()
        .expect("backlinks")
        .contains(&json!("metrics/gross-margin")));
    let computation = knowledge(
        &project,
        json!({"concept": "computations/revenue-ytd", "bundle": "acme_retail"}),
    )
    .await;
    assert_eq!(computation["attestable"], true, "{computation:#}");
    assert_eq!(computation["attestation_gaps"], json!([]));
    assert!(computation["field_links"]
        .as_array()
        .expect("field links")
        .contains(&json!({"field": "executor.resource", "id": "skills/run-on-bq"})));

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
async fn v0_2_bundles_keep_working_through_the_knowledge_tool() {
    let project = Project::new(&[("a.rs", "pub fn a() {}\n".into())]).await;
    store::import(
        project.dir.path(),
        &official("v0.2", "acme_retail"),
        &named("acme"),
    )
    .expect("import");
    // The v0.2 bundle-root path fields still resolve through the fallback.
    let computation = knowledge(
        &project,
        json!({"concept": "computations/revenue-ytd", "bundle": "acme"}),
    )
    .await;
    assert_eq!(computation["attestable"], true, "{computation:#}");
    assert!(computation["field_links"]
        .as_array()
        .expect("field links")
        .contains(&json!({"field": "executor.resource", "id": "skills/run-on-bq"})));
    // Comma-separated tags keep their earlier reading in legacy bundles.
    store::import(
        project.dir.path(),
        &official("v0.2", "stackoverflow"),
        &named("so"),
    )
    .expect("import");
    let dataset = knowledge(
        &project,
        json!({"query": "stackoverflow", "bundle": "so", "type": "BigQuery Dataset"}),
    )
    .await;
    let tags = dataset["results"][0]["tags"].as_array().expect("tags");
    assert!(tags.len() > 1, "{tags:?}");
}

#[tokio::test]
async fn v0_3_trust_lifecycle_and_extensions_reach_the_knowledge_tool() {
    let source = write_bundle(V03_BUNDLE);
    let project = Project::new(&[("a.rs", "pub fn a() {}\n".into())]).await;
    let outcome =
        store::import(project.dir.path(), source.path(), &named("finance")).expect("import");
    assert!(outcome.record.conformant);
    assert_eq!(outcome.record.declared_version.as_deref(), Some("0.3"));
    assert_eq!(
        (outcome.record.concepts, outcome.record.unreadable_concepts),
        (12, 2)
    );

    let listing = knowledge(&project, json!({})).await;
    let bundle = &listing["bundles"][0];
    assert_eq!(bundle["title"], "Finance knowledge");
    assert_eq!(bundle["license"], "/LICENSE.txt");
    assert_eq!(bundle["read_mode"], "current");

    let revenue = knowledge(
        &project,
        json!({"concept": "metrics/revenue", "bundle": "finance"}),
    )
    .await;
    assert_eq!(revenue["trust_tier"], "human-reviewed");
    assert_eq!(revenue["trust"]["current"][0]["by"], "human:ana");
    assert_eq!(revenue["freshness"], "fresh");
    assert_eq!(revenue["status"], "stable");
    // Unknown and namespaced extension keys survive at every level.
    let fm = &revenue["frontmatter"];
    assert_eq!(
        fm["x-acme-owner"],
        json!({"team": "team:fpa", "oncall": ["human:ana"]})
    );
    assert_eq!(fm["generated"]["x-acme-run"], 7);
    assert_eq!(
        fm["sources"][0]["x-acme-confidence"],
        json!({"level": "high", "reasons": ["audited", "current"]})
    );
    // A scalar `tags` value is one tag in a v0.3 bundle (§11.2).
    let hit = knowledge(
        &project,
        json!({"query": "recognized", "bundle": "finance"}),
    )
    .await;
    let first = &hit["results"][0];
    assert_eq!(first["id"], "metrics/revenue");
    assert_eq!(first["tags"], json!(["finance, revenue"]));
    // Links point only at concepts; broken ones are kept (§6.1). Both
    // spellings of the source path resolve to the same concept (§6).
    let links = revenue["links"].as_array().expect("links");
    assert_eq!(
        links,
        &vec![
            json!({"id": "computations/revenue", "exists": true}),
            json!({"id": "tables/orders", "exists": false}),
        ]
    );
    assert_eq!(
        revenue["field_links"],
        json!([
            {"field": "sources[0].resource", "id": "policies/recognition"},
            {"field": "sources[1].resource", "id": "policies/recognition"},
        ])
    );
    let policy = knowledge(
        &project,
        json!({"concept": "policies/recognition", "bundle": "finance"}),
    )
    .await;
    let backlinks = policy["backlinks"].as_array().expect("backlinks");
    assert!(
        backlinks.contains(&json!("metrics/revenue")),
        "{backlinks:?}"
    );
    // The bare root path of a v0.3 bundle is not an edge.
    assert!(!backlinks.contains(&json!("metrics/bare")), "{backlinks:?}");

    let rewritten = knowledge(
        &project,
        json!({"concept": "metrics/rewritten", "bundle": "finance"}),
    )
    .await;
    assert_eq!(rewritten["trust_tier"], "machine-confirmed");
    let earlier: Vec<&str> = rewritten["trust"]["earlier"]
        .as_array()
        .expect("earlier")
        .iter()
        .filter_map(|e| e["by"].as_str())
        .collect();
    assert_eq!(earlier, ["human:ana", "human:bob"]);
    assert_eq!(rewritten["freshness"], "stale");

    // Deprecated chains lead to the current concept; loops and dangling
    // replacements lead nowhere.
    for (id, replacement) in [
        ("metrics/old", json!("metrics/revenue")),
        ("metrics/older", json!("metrics/revenue")),
        ("metrics/loop-a", Value::Null),
        ("metrics/dangling", Value::Null),
    ] {
        let read = knowledge(&project, json!({"concept": id, "bundle": "finance"})).await;
        assert_eq!(read["replacement"], replacement, "{id}");
    }
    let current = knowledge(
        &project,
        json!({"query": "revenue", "bundle": "finance", "current_only": true, "type": "Metric"}),
    )
    .await;
    let ids = result_ids(&current);
    assert_eq!(
        ids.iter()
            .filter(|id| id.ends_with("metrics/revenue"))
            .count(),
        1,
        "{ids:?}"
    );
    for id in &ids {
        assert!(
            !["old", "older", "loop-a", "loop-b", "dangling", "draft"]
                .iter()
                .any(|gone| id.ends_with(&format!("metrics/{gone}"))),
            "{ids:?}"
        );
    }

    let attestable = knowledge(
        &project,
        json!({"concept": "computations/revenue", "bundle": "finance"}),
    )
    .await;
    assert_eq!(attestable["attestable"], true);
    let unchecked = knowledge(
        &project,
        json!({"concept": "computations/no-attester", "bundle": "finance"}),
    )
    .await;
    assert_eq!(unchecked["attestable"], false);
    assert_eq!(unchecked["attestation_gaps"], json!(["no_attester"]));

    // Files that are not concepts are kept but not searchable.
    let readme = knowledge(&project, json!({"query": "edit this bundle"})).await;
    assert_eq!(result_ids(&readme), Vec::<String>::new());
}

#[tokio::test]
async fn imported_bundles_round_trip_byte_for_byte() {
    // import -> stored copy -> import again: the stored copy is itself the
    // bundle, so metadata, links, provenance and extensions are unchanged.
    let source = write_bundle(V03_BUNDLE);
    let project = Project::new(&[("a.rs", "pub fn a() {}\n".into())]).await;
    let first = store::import(project.dir.path(), source.path(), &named("first")).expect("import");
    let stored = store::store_dir(project.dir.path())
        .join("first")
        .join(store::BUNDLE_DIR);
    for (path, content) in V03_BUNDLE {
        assert_eq!(
            std::fs::read(bundle::join(&stored, path)).expect("stored file"),
            content.to_vec(),
            "{path}"
        );
    }
    let second = store::import(project.dir.path(), &stored, &named("second")).expect("re-import");
    assert_eq!(first.report.diagnostics, second.report.diagnostics);
    assert_eq!(first.record.files, second.record.files);
    assert_eq!(first.record.bytes, second.record.bytes);
    let a = knowledge(
        &project,
        json!({"concept": "metrics/revenue", "bundle": "first"}),
    )
    .await;
    let b = knowledge(
        &project,
        json!({"concept": "metrics/revenue", "bundle": "second"}),
    )
    .await;
    for key in [
        "frontmatter",
        "body",
        "links",
        "field_links",
        "trust",
        "trust_tier",
    ] {
        assert_eq!(a[key], b[key], "{key}");
    }
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
    assert_eq!(outcome.record.declared_version.as_deref(), Some("0.3"));

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

#[tokio::test]
async fn bundles_exported_before_v0_3_still_map_to_project_files() {
    // Exports written before OKF v0.3 support used `tokensave_path`.
    let project = Project::new(&[("src/util.rs", "pub fn helper() {}\n".into())]).await;
    let old = write_bundle(&[
        ("index.md", b"---\nokf_version: \"0.2\"\n---\n"),
        (
            "files/src/util.rs.md",
            b"---\ntype: \"Source File\"\ntokensave_path: \"src/util.rs\"\n---\n",
        ),
    ]);
    store::import(project.dir.path(), old.path(), &named("old")).expect("import");
    let by_file = knowledge(&project, json!({"file": "src/util.rs"})).await;
    assert_eq!(result_ids(&by_file), ["old:files/src/util.rs"]);
}
