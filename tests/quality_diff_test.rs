use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};
use tempfile::{tempdir, TempDir};
use tokensave::{
    branch_meta,
    mcp::tools::handle_tool_call,
    quality_diff::{compare, QualityOptions},
    tokensave::TokenSave,
};

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
fn source(branches: usize) -> String {
    format!(
        "pub fn example(x: i32) {{\n{}\n}}\n",
        (0..branches)
            .map(|n| format!("if x == {n} {{ println!(\"yes\"); }}\n"))
            .collect::<String>()
    )
}
async fn fixture() -> TempDir {
    let dir = tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-b", "master"]);
    fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    fs::write(root.join("base.rs"), source(12)).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    git(root, &["switch", "-c", "feature"]);
    let td = root.join(".tokensave");
    fs::create_dir_all(td.join("branches")).unwrap();
    fs::copy(
        td.join("tokensave.db"),
        td.join("branches").join("feature.db"),
    )
    .unwrap();
    let mut meta = branch_meta::load_branch_meta(&td).unwrap();
    meta.add_branch("feature", "branches/feature.db", "master");
    branch_meta::save_branch_meta(&td, &meta).unwrap();
    dir
}
async fn update(root: &Path, n: usize) {
    fs::write(root.join("base.rs"), source(n)).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "change"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
}
fn cli(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(root)
        .arg("quality-diff")
        .args(args)
        .output()
        .unwrap()
}

#[tokio::test]
async fn baseline_lifecycle_json_read_only_and_ci() {
    let dir = fixture().await;
    let root = dir.path();
    let options = QualityOptions::default();
    let unchanged = compare(root, "master", "feature", &options).await.unwrap();
    assert!(!unchanged.has_regressions());
    assert!(unchanged.summary["unchanged"] > 0);
    assert!(unchanged
        .findings
        .iter()
        .all(|f| f.base_value == f.target_value));
    update(root, 20).await;
    fs::write(
        root.join("base.rs"),
        "// unsaved local work, preserved by comparison\n",
    )
    .unwrap();
    let before = git(root, &["status", "--porcelain=v1", "--untracked-files=all"]);
    let head = git(root, &["rev-parse", "HEAD"]);
    let db_before = fs::read(root.join(".tokensave").join("branches").join("feature.db")).unwrap();
    let meta_before = fs::read(root.join(".tokensave").join("branch-meta.json")).unwrap();
    let report = compare(root, "master", "HEAD", &options).await.unwrap();
    assert!(report.summary["worsened"] > 0);
    assert_eq!(report.exit_code(true), 1);
    assert_eq!(report.exit_code(false), 0);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == "Q_COMPLEXITY_CYCLOMATIC"
            && f.base_value == Some(13)
            && f.target_value == Some(21)));
    let repeated = compare(root, "master", "HEAD", &options).await.unwrap();
    assert_eq!(
        serde_json::to_string(&report).unwrap(),
        serde_json::to_string(&repeated).unwrap()
    );
    let out = cli(
        root,
        &[
            "--base",
            "master",
            "--target",
            "HEAD",
            "--json",
            "--fail-on",
            "regression",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(parsed["complete"], true);
    assert_eq!(
        git(root, &["status", "--porcelain=v1", "--untracked-files=all"]),
        before
    );
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read(root.join(".tokensave").join("branches").join("feature.db")).unwrap(),
        db_before
    );
    assert_eq!(
        fs::read(root.join(".tokensave").join("branch-meta.json")).unwrap(),
        meta_before
    );
    assert_eq!(
        fs::read_to_string(root.join("base.rs")).unwrap(),
        "// unsaved local work, preserved by comparison\n"
    );
    let resolved = compare(root, "feature", "master", &options).await.unwrap();
    assert!(resolved.summary["improved"] > 0);
    assert!(!resolved.has_regressions());
    update(root, 0).await;
    let resolved = compare(root, "master", "feature", &options).await.unwrap();
    assert!(resolved.summary["resolved"] > 0);
    assert!(!resolved.has_regressions());
}

#[tokio::test]
async fn stale_missing_invalid_refs_and_mcp() {
    let dir = fixture().await;
    let root = dir.path();
    let options = QualityOptions::default();
    assert!(compare(root, "--help", "feature", &options).await.is_err());
    git(root, &["branch", "unindexed"]);
    assert!(compare(root, "unindexed", "feature", &options)
        .await
        .unwrap_err()
        .to_string()
        .contains("not tracked"));
    // A branch tip advanced without updating its index, even in a same-ref comparison.
    fs::write(root.join("base.rs"), source(20)).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "not indexed"]);
    assert!(compare(root, "feature", "feature", &options)
        .await
        .unwrap_err()
        .to_string()
        .contains("stale"));
    let out = cli(
        root,
        &["--base", "feature", "--target", "feature", "--json"],
    );
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["complete"],
        false
    );
    let stale = compare(
        root,
        "master",
        "feature",
        &QualityOptions {
            allow_stale: true,
            ..options.clone()
        },
    )
    .await
    .unwrap();
    assert!(!stale.complete);
    assert_eq!(stale.exit_code(false), 2);
    let cg = TokenSave::open_read_only(root, Some("master"))
        .await
        .unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_branch_diff",
        json!({"base":"master","head":"master","quality":true,"limit":0}),
        None,
        None,
    )
    .await
    .unwrap();
    let v: Value =
        serde_json::from_str(result.value["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(v["report"]["summary"]["new"], 0);
    assert!(v["total_findings"].as_u64().unwrap() > 0);
    assert_eq!(v["returned_findings"], 0);
    assert_eq!(v["truncated"], true);
    assert!(handle_tool_call(
        &cg,
        "tokensave_branch_diff",
        json!({"base":"master","head":"master","quality":true,"thresholds":{"bogus":1}}),
        None,
        None
    )
    .await
    .is_err());
}

#[tokio::test]
async fn file_rename_remote_ref_divergence_and_deletion() {
    let dir = fixture().await;
    let root = dir.path();
    // Reproduce Windows checkout conversion on every platform. After switching
    // back to master, base.rs is CRLF on disk but LF in the committed Git blob.
    git(root, &["config", "core.autocrlf", "true"]);
    git(root, &["mv", "base.rs", "moved.rs"]);
    git(root, &["commit", "-m", "rename"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let options = QualityOptions::default();
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert!(!report.has_regressions(), "{}", report.human());
    assert!(report.summary["unchanged"] > 0);
    // Index a valid remote-tracking reference under its own exact name.
    git(
        root,
        &["update-ref", "refs/remotes/origin/feature", "feature"],
    );
    let td = root.join(".tokensave");
    let mut meta = branch_meta::load_branch_meta(&td).unwrap();
    meta.add_branch("origin/feature", "branches/feature.db", "master");
    branch_meta::save_branch_meta(&td, &meta).unwrap();
    assert!(!compare(
        root,
        "refs/heads/master",
        "refs/remotes/origin/feature",
        &options
    )
    .await
    .unwrap()
    .has_regressions());
    // Advance master separately: the comparison remains explicitly tip-to-tip.
    git(root, &["switch", "master"]);
    fs::write(root.join("other.rs"), "pub fn other() {}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "diverge"]);
    let cg = TokenSave::open_branch(root, "master").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    assert!(fs::read(root.join("base.rs"))
        .unwrap()
        .windows(2)
        .any(|bytes| bytes == b"\r\n"));
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert_eq!(report.baseline, "branch_tips");
    assert!(report.complete);
    assert!(!report.has_regressions());
    git(root, &["switch", "feature"]);
    git(root, &["rm", "moved.rs"]);
    fs::write(root.join("empty.rs"), "pub fn empty() {}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "delete"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert!(report.summary["resolved"] > 0);
    assert!(!report.has_regressions());
}

#[tokio::test]
async fn crlf_index_accepts_git_normalization_but_rejects_stale_source() {
    let dir = fixture().await;
    let root = dir.path();
    git(root, &["config", "core.autocrlf", "input"]);
    // Git commits LF, while the index sees the CRLF source written by an editor.
    fs::write(root.join("base.rs"), source(20).replace('\n', "\r\n")).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "normalized source"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let options = QualityOptions::default();
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert!(report.complete);
    assert!(report.summary["worsened"] > 0);
    let out = cli(
        root,
        &["--base", "feature", "--target", "feature", "--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["complete"],
        true
    );
    // A same-size source edit must still fail, even with equivalent line endings.
    fs::write(
        root.join("base.rs"),
        source(20)
            .replace("example", "changed")
            .replace('\n', "\r\n"),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "stale source"]);
    let err = compare(root, "master", "feature", &options)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("stale indexed contents: base.rs"));
}

#[tokio::test]
async fn cycles_coupling_and_preexisting_edges() {
    let dir = fixture().await;
    let root = dir.path();
    // Use known extracted symbols and explicit resolved calls to test graph semantics
    // independently of resolver heuristics.
    for (branch, extra_cycle) in [("master", false), ("feature", true)] {
        git(root, &["switch", branch]);
        fs::write(root.join("a.rs"), "pub fn a() {}\n").unwrap();
        fs::write(root.join("b.rs"), "pub fn b() {}\n").unwrap();
        fs::write(root.join("c.rs"), "pub fn c() {}\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "graph"]);
        let cg = TokenSave::open_branch(root, branch).await.unwrap();
        cg.index_all().await.unwrap();
        let a = cg
            .get_nodes_by_file("a.rs")
            .await
            .unwrap()
            .into_iter()
            .find(|n| n.name == "a")
            .unwrap();
        let b = cg
            .get_nodes_by_file("b.rs")
            .await
            .unwrap()
            .into_iter()
            .find(|n| n.name == "b")
            .unwrap();
        cg.db()
            .conn()
            .execute(
                "INSERT INTO edges (source,target,kind) VALUES (?1,?2,'calls')",
                libsql::params![a.id.clone(), b.id.clone()],
            )
            .await
            .unwrap();
        cg.db()
            .conn()
            .execute(
                "INSERT INTO edges (source,target,kind) VALUES (?1,?2,'calls')",
                libsql::params![b.id.clone(), a.id],
            )
            .await
            .unwrap();
        if extra_cycle {
            let c = cg
                .get_nodes_by_file("c.rs")
                .await
                .unwrap()
                .into_iter()
                .find(|n| n.name == "c")
                .unwrap();
            cg.db()
                .conn()
                .execute(
                    "INSERT INTO edges (source,target,kind) VALUES (?1,?2,'calls')",
                    libsql::params![b.id.clone(), c.id.clone()],
                )
                .await
                .unwrap();
            cg.db()
                .conn()
                .execute(
                    "INSERT INTO edges (source,target,kind) VALUES (?1,?2,'calls')",
                    libsql::params![c.id, b.id],
                )
                .await
                .unwrap();
        }
        cg.checkpoint().await.unwrap();
    }
    let options = QualityOptions {
        fan_out: 0,
        min_increase: 1,
        ..QualityOptions::default()
    };
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.code == "Q_CYCLIC_DEPENDENCY_EDGE" && f.status == "new")
            .count(),
        2
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.category == "coupling" && f.status == "new"));
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.code == "Q_CYCLIC_DEPENDENCY_EDGE" && f.status == "unchanged")
            .count(),
        2
    );
    let same = compare(root, "feature", "feature", &options).await.unwrap();
    assert!(!same.has_regressions());
    assert_eq!(
        same.findings
            .iter()
            .filter(|f| f.code == "Q_CYCLIC_DEPENDENCY_EDGE" && f.status == "unchanged")
            .count(),
        4
    );
    let reverse = compare(root, "feature", "master", &options).await.unwrap();
    assert_eq!(
        reverse
            .findings
            .iter()
            .filter(|f| f.code == "Q_CYCLIC_DEPENDENCY_EDGE" && f.status == "resolved")
            .count(),
        2
    );
}

#[tokio::test]
async fn missing_analysis_candidates_symbol_rename_and_small_increases() {
    let dir = fixture().await;
    let root = dir.path();
    let options = QualityOptions::default();
    // A new supported file not represented in an otherwise unchanged index is not clean.
    fs::write(root.join("new.rs"), "pub fn missing() {}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "missing analysis"]);
    let err = compare(root, "master", "feature", &options)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("unindexed source candidate"), "{err}");
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    // Source locations alone never create new issue identities.
    fs::write(root.join("base.rs"), format!("\n\n{}", source(13))).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "small increase and location"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == "Q_COMPLEXITY_CYCLOMATIC"
            && f.status == "unchanged"
            && f.base_value == Some(13)
            && f.target_value == Some(14)));
    let strict = compare(
        root,
        "master",
        "feature",
        &QualityOptions {
            min_increase: 1,
            ..options.clone()
        },
    )
    .await
    .unwrap();
    assert!(strict.summary["worsened"] > 0);
    fs::write(
        root.join("base.rs"),
        source(12).replace("example", "renamed"),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "symbol rename"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let report = compare(root, "master", "feature", &options).await.unwrap();
    assert!(report
        .findings
        .iter()
        .any(|f| f.status == "new" && f.confidence == "unmatched"));
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.db()
        .set_metadata("last_full_index_version", "different-analysis-version")
        .await
        .unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    assert!(compare(root, "master", "feature", &options)
        .await
        .unwrap_err()
        .to_string()
        .contains("version provenance"));
    let partial = compare(
        root,
        "master",
        "feature",
        &QualityOptions {
            allow_stale: true,
            ..options.clone()
        },
    )
    .await
    .unwrap();
    assert!(!partial.complete);
    // A deleted indexed blob is stale evidence, not a successful resolution.
    git(root, &["rm", "base.rs"]);
    git(root, &["commit", "-m", "stale deletion"]);
    assert!(compare(root, "master", "feature", &options)
        .await
        .unwrap_err()
        .to_string()
        .contains("absent at branch tip"));
    fs::remove_file(root.join(".tokensave").join("branches").join("feature.db")).unwrap();
    assert!(compare(root, "master", "feature", &options)
        .await
        .unwrap_err()
        .to_string()
        .contains("missing"));
}

#[tokio::test]
async fn large_index_and_empty_analysis() {
    let dir = fixture().await;
    let root = dir.path();
    let source: String = (0..5000)
        .map(|n| format!("pub fn function_{n}() {{}}\n"))
        .collect();
    fs::write(root.join("base.rs"), source).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "large index"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let start = std::time::Instant::now();
    let report = compare(root, "feature", "feature", &QualityOptions::default())
        .await
        .unwrap();
    eprintln!(
        "5000-function identical-index comparison: {:?}",
        start.elapsed()
    );
    assert!(!report.has_regressions());
    assert!(report.findings.is_empty());
    assert!(report.complete);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.db()
        .conn()
        .execute("DELETE FROM files", ())
        .await
        .unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    assert!(
        compare(root, "feature", "feature", &QualityOptions::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("no files")
    );
}

#[tokio::test]
async fn ambiguous_variants_are_incomplete_and_nested_projects_read_git_blobs() {
    let dir = fixture().await;
    let root = dir.path();
    fs::write(
        root.join("base.rs"),
        format!(
            "#[cfg(unix)]\n{}\n#[cfg(windows)]\n{}",
            source(12),
            source(12)
        ),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "variants"]);
    let cg = TokenSave::open_branch(root, "feature").await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let report = compare(root, "feature", "feature", &QualityOptions::default())
        .await
        .unwrap();
    assert!(!report.has_regressions());
    assert!(!report.complete);
    assert!(report
        .limitations
        .iter()
        .any(|s| s.contains("ambiguous metrics")));
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("lib.rs"), source(0)).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "nested project"]);
    let cg = TokenSave::init(&nested).await.unwrap();
    cg.index_all().await.unwrap();
    cg.checkpoint().await.unwrap();
    drop(cg);
    let report = compare(&nested, "feature", "feature", &QualityOptions::default())
        .await
        .unwrap();
    assert!(report.complete);
    assert!(!report.has_regressions());
}
