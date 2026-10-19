//! Bounded metamorphic tests. Fixtures are valid source, not compiled programs.
use crate::common::stability::{entity, Correspondence, Project, Snapshot};

fn fixture(language: &str) -> Vec<(&'static str, String)> {
    match language {
        "rust" => vec![
            ("src/lib.rs", "mod a;\nmod b;\n".into()),
            ("src/a.rs", "use crate::b::beta;\n\npub fn alpha(n: i32) -> i32 {\n    if n > 0 {\n        return beta(n - 1);\n    }\n    0\n}\n\nfn unused() -> i32 {\n    42\n}\n".into()),
            ("src/b.rs", "use crate::a::alpha;\n\npub fn beta(n: i32) -> i32 {\n    alpha(n)\n}\n".into()),
        ],
        "python" => vec![
            ("a.py", "import b\n\ndef alpha(n):\n    if n > 0:\n        return b.beta(n - 1)\n    return 0\n\ndef unused():\n    return 42\n".into()),
            ("b.py", "import a\n\ndef beta(n):\n    return a.alpha(n)\n".into()),
        ],
        _ => unreachable!(),
    }
}

fn transform(source: &str, language: &str, transformation: &str) -> String {
    match transformation {
        "blank_lines" => source.replace('\n', "\n\n"),
        "indentation" => source
            .lines()
            .map(|line| {
                let spaces = line.len() - line.trim_start_matches(' ').len();
                format!("{}{}\n", " ".repeat(spaces * 2), &line[spaces..])
            })
            .collect(),
        "comments" => {
            let comment = if language == "rust" {
                "// ordinary comment"
            } else {
                "# ordinary comment"
            };
            format!("{comment}\n{source}\n{comment}\n")
        }
        "crlf" => source.replace('\n', "\r\n"),
        "reorder" => {
            // Only independent top-level functions; imports stay before definitions.
            let marker = if language == "rust" {
                "fn unused"
            } else {
                "def unused"
            };
            if let Some(offset) = source.find(marker) {
                let first = source
                    .find(if language == "rust" {
                        "pub fn"
                    } else {
                        "def alpha"
                    })
                    .unwrap();
                format!(
                    "{}{}\n{}",
                    &source[..first],
                    &source[offset..],
                    &source[first..offset]
                )
            } else {
                source.into()
            }
        }
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn semantics_preserving_transformations() {
    for language in ["rust", "python"] {
        let files = fixture(language);
        let project = Project::new(&files).await;
        let baseline = project.snapshot().await;
        // Protect against vacuous success if resolution or an analyzer stops working.
        assert_eq!(baseline.analyses["symbols"].len(), 3);
        assert!(baseline.analyses["dead_code"]
            .iter()
            .any(|s| s.contains("unused")));
        assert_eq!(baseline.analyses["circular_dependencies"].len(), 1);
        assert_eq!(baseline.analyses["coupling_in"].len(), 2);
        assert_eq!(baseline.analyses["coupling_out"].len(), 2);
        assert!(baseline.analyses["relationships"]
            .iter()
            .any(|s| s.contains("Calls")));
        for transformation in ["blank_lines", "indentation", "comments", "crlf", "reorder"] {
            let changed: Vec<_> = files
                .iter()
                .map(|(p, s)| (*p, transform(s, language, transformation)))
                .collect();
            Project::write(project.dir.path(), &changed);
            project.cg.sync().await.unwrap();
            let incremental = project.snapshot().await;
            let context = format!("{language}/{transformation}");
            baseline.assert_equivalent(&incremental, &context);
            if transformation == "blank_lines" {
                assert_ne!(
                    baseline.identities, incremental.identities,
                    "line-derived raw IDs should change; semantic finding identities should not"
                );
            }
            let clean = Project::new(&changed).await;
            let full = clean.snapshot().await;
            incremental.assert_same_identities(&full, &context);
            incremental.assert_equivalent(&full, &format!("{context}/incremental vs clean full"));
            // Explicit reverse transformation covers comment removal and stale-edge cleanup.
            Project::write(project.dir.path(), &files);
            project.cg.sync().await.unwrap();
            baseline.assert_equivalent(&project.snapshot().await, &format!("{context}/restore"));
        }
    }
}

#[tokio::test]
async fn identical_inputs_and_concurrent_readers() {
    let project = Project::new(&fixture("rust")).await;
    let baseline = project.snapshot().await;
    let mut exact_before = project.cg.get_all_nodes().await.unwrap();
    let mut exact_after = project.cg.get_all_nodes().await.unwrap();
    exact_before.sort_by(|a, b| a.id.cmp(&b.id));
    exact_after.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        exact_before, exact_after,
        "exact node output equality without writes"
    );
    for run in 0..3 {
        project.cg.sync().await.unwrap();
        let after = project.snapshot().await;
        baseline.assert_equivalent(&after, &format!("no-op sync {run}"));
        baseline.assert_same_identities(&after, "unchanged inputs");
        project.cg.index_all().await.unwrap();
        let full = project.snapshot().await;
        baseline.assert_equivalent(&full, "repeat full index");
        baseline.assert_same_identities(&full, "repeat full index");
    }
    // Bounded concurrent reads; no sleeps, shared mutations or global configuration.
    let (a, b) = tokio::join!(project.snapshot(), project.snapshot());
    baseline.assert_equivalent(&a, "concurrent reader A");
    baseline.assert_equivalent(&b, "concurrent reader B");
    baseline.assert_same_identities(&a, "concurrent reads");
}

#[tokio::test]
async fn controlled_rename_has_explicit_finding_correspondence() {
    let files = fixture("rust");
    let project = Project::new(&files).await;
    let baseline = project.snapshot().await;
    let old_nodes = project.cg.get_all_nodes().await.unwrap();
    let changed: Vec<_> = files
        .iter()
        .map(|(p, s)| {
            (
                *p,
                s.replace("unused", "renamed_unused")
                    .replace("beta", "renamed_beta"),
            )
        })
        .collect();
    Project::write(project.dir.path(), &changed);
    project.cg.sync().await.unwrap();
    let unmapped = project.snapshot().await;
    assert_ne!(
        baseline.analyses["dead_code"],
        unmapped.analyses["dead_code"]
    );
    let new_nodes = project.cg.get_all_nodes().await.unwrap();
    let old = old_nodes.iter().find(|n| n.name == "unused").unwrap();
    let new = new_nodes
        .iter()
        .find(|n| n.name == "renamed_unused")
        .unwrap();
    assert_ne!(old.id, new.id, "name participates in raw identity");
    let mut correspondence = Correspondence::default();
    correspondence.entities.insert(entity(new), entity(old));
    let old_callee = old_nodes.iter().find(|n| n.name == "beta").unwrap();
    let new_callee = new_nodes.iter().find(|n| n.name == "renamed_beta").unwrap();
    correspondence
        .entities
        .insert(entity(new_callee), entity(old_callee));

    let old_import = old_nodes
        .iter()
        .find(|n| n.qualified_name == "src/a.rs::crate::b::beta")
        .unwrap();
    let new_import = new_nodes
        .iter()
        .find(|n| n.qualified_name == "src/a.rs::crate::b::renamed_beta")
        .unwrap();
    correspondence
        .entities
        .insert(entity(new_import), entity(old_import));
    for (new, old) in [(new, old), (new_callee, old_callee)] {
        correspondence
            .signatures
            .insert(entity(new), old.signature.clone());
    }
    let mapped = Snapshot::capture(&project.cg, &correspondence).await;
    baseline.assert_equivalent(
        &mapped,
        "controlled rename unused/beta -> renamed_unused/renamed_beta",
    );
    let clean = Project::new(&changed).await;
    mapped.assert_equivalent(
        &Snapshot::capture(&clean.cg, &correspondence).await,
        "rename incremental vs clean",
    );
}

#[tokio::test]
async fn meaningful_changes_are_not_normalized_away() {
    let files = fixture("rust");
    let project = Project::new(&files).await;
    let baseline = project.snapshot().await;
    let changed: Vec<_> = files
        .iter()
        .map(|(p, s)| {
            (
                *p,
                s.replace("if n > 0 {", "if n > 0 && n < 100 {")
                    .replace("use crate::a::alpha;\n", "")
                    .replace("alpha(n)\n", "n\n"),
            )
        })
        .collect();
    Project::write(project.dir.path(), &changed);
    project.cg.sync().await.unwrap();
    let after = project.snapshot().await;
    assert_eq!(baseline.analyses["symbols"], after.analyses["symbols"]);
    for analyzer in [
        "complexity",
        "relationships",
        "dead_code",
        "circular_dependencies",
        "coupling_in",
        "coupling_out",
    ] {
        assert_ne!(
            baseline.analyses[analyzer], after.analyses[analyzer],
            "negative control must detect changed {analyzer}"
        );
    }
    let clean = Project::new(&changed).await;
    after.assert_equivalent(
        &clean.snapshot().await,
        "semantic change incremental vs clean",
    );
}

#[tokio::test]
async fn independent_insertion_preserves_existing_findings() {
    let files = fixture("rust");
    let project = Project::new(&files).await;
    let baseline = project.snapshot().await;
    let changed: Vec<_> = files
        .iter()
        .map(|(p, s)| {
            (
                *p,
                if *p == "src/b.rs" {
                    format!("{s}\nfn independent() -> i32 {{ 7 }}\n")
                } else {
                    s.clone()
                },
            )
        })
        .collect();
    Project::write(project.dir.path(), &changed);
    project.cg.sync().await.unwrap();
    let mut after = project.snapshot().await;
    let clean = Project::new(&changed).await;
    after.assert_equivalent(
        &clean.snapshot().await,
        "independent insertion incremental vs clean",
    );
    let nodes = project.cg.get_all_nodes().await.unwrap();
    let added = nodes.iter().find(|n| n.name == "independent").unwrap();
    let key = entity(added);
    // Account for exactly one expected addition to each applicable analyzer.
    // No blanket subset comparison: every other row must remain identical.
    for analyzer in ["symbols", "complexity", "dead_code"] {
        let rows = after.analyses.get_mut(analyzer).unwrap();
        assert_eq!(rows.len(), baseline.analyses[analyzer].len() + 1);
        let prefix = format!("{key:?}");
        let index = rows
            .iter()
            .position(|row| row.starts_with(&prefix))
            .unwrap();
        rows.remove(index);
    }
    after.identities.remove(&key).unwrap();
    baseline.assert_equivalent(&after, "independent insertion existing findings");
    baseline.assert_same_identities(&after, "insertion after existing declarations");
    let root = project.dir.path().to_path_buf();
    drop(project);
    assert!(
        !root.exists(),
        "fixture directory and database must be cleaned up"
    );
}
