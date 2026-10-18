//! Read-only, tip-to-tip comparison of existing branch quality indexes.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::errors::{Result, TokenSaveError};
use crate::graph::{queries::GraphQueryManager, scc};
use crate::tokensave::TokenSave;
use crate::types::NodeKind;

fn error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

/// Thresholds use existing extracted metrics, not inferred source complexity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QualityOptions {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub function_lines: u32,
    pub fan_out: u32,
    pub min_increase: u32,
    /// Explicit opt-in to compare stale contents, with an incomplete result.
    pub allow_stale: bool,
}
impl Default for QualityOptions {
    fn default() -> Self {
        Self {
            cyclomatic: 10,
            cognitive: 15,
            function_lines: 100,
            fan_out: 10,
            min_increase: 3,
            allow_stale: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: String,
    pub category: String,
    pub severity: String,
    pub identity: String,
    pub file: String,
    pub symbol: Option<String>,
    /// One-based location; absent for file-level diagnostics.
    pub line: Option<u32>,
    pub base_value: Option<u32>,
    pub target_value: Option<u32>,
    pub status: String,
    pub confidence: String,
    pub explanation: String,
}
#[derive(Debug, Serialize)]
pub struct QualityReport {
    pub base: String,
    pub target: String,
    pub base_commit: String,
    pub target_commit: String,
    pub baseline: String,
    pub complete: bool,
    pub summary: BTreeMap<String, usize>,
    pub findings: Vec<Finding>,
    pub limitations: Vec<String>,
}
impl QualityReport {
    pub fn has_regressions(&self) -> bool {
        self.summary.get("new").copied().unwrap_or(0)
            + self.summary.get("worsened").copied().unwrap_or(0)
            > 0
    }
    /// 0 success, 1 qualifying regression, 2 incomplete/operational failure.
    pub fn exit_code(&self, fail_on_regression: bool) -> i32 {
        if self.complete {
            i32::from(fail_on_regression && self.has_regressions())
        } else {
            2
        }
    }
    pub fn human(&self) -> String {
        let mut text = format!(
            "Tokensave Code Quality Comparison\nBase: {}\nTarget: {}\nBaseline: branch tips\n",
            self.base, self.target
        );
        for (status, count) in &self.summary {
            let _ = writeln!(text, "{status}: {count}");
        }
        for f in self.findings.iter().filter(|f| f.status != "unchanged") {
            let _ = writeln!(
                text,
                "{} {} {} {} {:?} -> {:?}: {}",
                f.status,
                f.code,
                f.file,
                f.symbol.as_deref().unwrap_or(""),
                f.base_value,
                f.target_value,
                f.explanation
            );
        }
        for note in &self.limitations {
            let _ = writeln!(text, "Note: {note}");
        }
        text.push_str(if !self.complete {
            "Overall result: INCOMPLETE\n"
        } else if self.has_regressions() {
            "Overall result: REGRESSIONS DETECTED\n"
        } else {
            "Overall result: NO REGRESSIONS DETECTED\n"
        });
        text
    }
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| error(format!("cannot run git: {e}")))?;
    if !output.status.success() {
        return Err(error(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}
fn resolve(root: &Path, reference: &str) -> Result<(String, String)> {
    // --end-of-options prevents option injection, including refs beginning '-'.
    let oid = String::from_utf8(git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
    )?)
    .map_err(|_| error("invalid Git object id"))?
    .trim()
    .to_string();
    let name = if reference == "HEAD" {
        String::from_utf8(git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?)
            .map_err(|_| error("invalid branch name"))?
            .trim()
            .to_string()
    } else {
        reference
            .strip_prefix("refs/heads/")
            .or_else(|| reference.strip_prefix("refs/remotes/"))
            .unwrap_or(reference)
            .to_string()
    };
    Ok((name, oid))
}

/// A batch process is always reaped, including early stale-index/error returns.
struct BlobReader(std::process::Child);
impl Drop for BlobReader {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn verify(cg: &TokenSave, commit: &str) -> Result<Vec<String>> {
    let prefix = String::from_utf8(git(cg.project_root(), &["rev-parse", "--show-prefix"])?)
        .map_err(|_| error("non-UTF-8 project prefix"))?;
    let prefix = prefix.trim_end_matches('\n');
    let files = cg.get_all_files().await?;
    if files.is_empty() {
        return Err(error("index has no files; analysis data is unavailable"));
    }
    let indexed: BTreeSet<_> = files.iter().map(|f| f.path.as_str()).collect();
    let tree = git(
        cg.project_root(),
        &["ls-tree", "-r", "-z", "--long", commit, "--"],
    )?;
    let registry = crate::extraction::LanguageRegistry::new();
    let extensions = registry.supported_extensions();
    let config = cg.get_config();
    let mut stale = Vec::new();
    // Detect potentially missing analysis rather than declaring an incomplete index clean.
    // Reuse configured exclusions; Git ignore/manifest policies can differ between tips,
    // so unindexed candidates are deliberately reported as uncertain evidence.
    for entry in tree.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let Some(tab) = entry.iter().position(|b| *b == b'\t') else {
            continue;
        };
        let path = std::str::from_utf8(&entry[tab + 1..])
            .map_err(|_| error("non-UTF-8 Git paths are unsupported"))?;
        if indexed.contains(path)
            || crate::config::is_excluded(path, config)
            || Path::new(path)
                .ancestors()
                .skip(1)
                .any(|p| crate::config::is_excluded_dir(&p.to_string_lossy(), config))
        {
            continue;
        }
        let hidden = path.split('/').any(|part| part.starts_with('.'));
        if hidden
            && !crate::config::is_included(path, config)
            && !crate::config::is_force_included(path, config)
        {
            continue;
        }
        let header =
            std::str::from_utf8(&entry[..tab]).map_err(|_| error("invalid Git tree header"))?;
        let size = header
            .split_whitespace()
            .nth(3)
            .and_then(|v| v.parse::<u64>().ok());
        if size.is_none_or(|size| size > config.max_file_size) {
            continue;
        }
        let extension = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if extensions.contains(&extension) {
            stale.push(format!(
                "unindexed source candidate (ignore/manifest exclusions may explain it): {path}"
            ));
        }
    }
    let mut child = BlobReader(
        Command::new("git")
            .arg("-C")
            .arg(cg.project_root())
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| error(format!("cannot read Git blobs: {e}")))?,
    );
    let mut input = child
        .0
        .stdin
        .take()
        .ok_or_else(|| error("missing git stdin"))?;
    let mut output = BufReader::new(
        child
            .0
            .stdout
            .take()
            .ok_or_else(|| error("missing git stdout"))?,
    );
    for file in files {
        if file.path.contains(['\n', '\r']) {
            return Err(error("newline in indexed path is unsupported"));
        }
        writeln!(input, "{commit}:{prefix}{}", file.path)
            .and_then(|()| input.flush())
            .map_err(|e| error(e.to_string()))?;
        let mut header = String::new();
        output
            .read_line(&mut header)
            .map_err(|e| error(e.to_string()))?;
        if header.ends_with(" missing\n") {
            stale.push(format!("indexed file absent at branch tip: {}", file.path));
            continue;
        }
        let size = header
            .split_whitespace()
            .nth(2)
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| error("invalid git cat-file response"))?;
        // Hash one decoded source at a time. Refuse oversized blobs instead of unbounded allocation.
        if size > 64 * 1024 * 1024 {
            return Err(error(format!(
                "indexed blob exceeds 64 MiB verification limit: {}",
                file.path
            )));
        }
        let mut bytes = vec![0; size as usize];
        output
            .read_exact(&mut bytes)
            .map_err(|e| error(e.to_string()))?;
        let mut newline = [0];
        output
            .read_exact(&mut newline)
            .map_err(|e| error(e.to_string()))?;
        let source = crate::sync::decode_source_bytes(&bytes).map_err(|e| error(e.to_string()))?;
        if crate::sync::content_hash(&source) != file.content_hash {
            stale.push(format!("stale indexed contents: {}", file.path));
        }
    }
    stale.sort();
    Ok(stale)
}

#[derive(Clone)]
struct Metric {
    code: &'static str,
    category: &'static str,
    file: String,
    symbol: Option<String>,
    line: Option<u32>,
    value: u32,
    threshold: u32,
    ambiguous: bool,
}
type Snapshot = BTreeMap<(String, String), Metric>;
fn insert(snapshot: &mut Snapshot, identity: String, metric: Metric) {
    let key = (metric.code.into(), identity);
    match snapshot.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(metric);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            entry.get_mut().ambiguous = true;
        }
    }
}

async fn snapshot(
    cg: &TokenSave,
    options: &QualityOptions,
    renames: &BTreeMap<String, String>,
) -> Result<Snapshot> {
    let mut result = Snapshot::new();
    let canonical = |path: &str| {
        renames
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_string())
    };
    // File batches avoid holding all source nodes (and their docstrings) at once.
    for file in cg.get_all_files().await? {
        let nodes = cg.get_nodes_by_file(&file.path).await?;
        let mut counts = BTreeMap::new();
        for n in &nodes {
            *counts
                .entry((n.kind.as_str(), n.qualified_name.as_str()))
                .or_insert(0_usize) += 1;
        }
        for n in &nodes {
            if !matches!(
                n.kind,
                NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::SingletonMethod
                    | NodeKind::ArrowFunction
            ) {
                continue;
            }
            let path = canonical(&n.file_path);
            let local_name = n
                .qualified_name
                .strip_prefix(&n.file_path)
                .unwrap_or(&n.qualified_name);
            let mut identity = format!("{path}:{}:{local_name}", n.kind.as_str());
            if counts[&(n.kind.as_str(), n.qualified_name.as_str())] > 1 {
                identity.push_str(
                    &serde_json::to_string(&n.signature).map_err(|e| error(e.to_string()))?,
                );
            }
            for (code, category, value, threshold) in [
                (
                    "Q_COMPLEXITY_CYCLOMATIC",
                    "complexity",
                    n.branches.saturating_add(1),
                    options.cyclomatic,
                ),
                (
                    "Q_COMPLEXITY_COGNITIVE",
                    "complexity",
                    n.cognitive_complexity,
                    options.cognitive,
                ),
                (
                    "Q_FUNCTION_LINES",
                    "maintainability",
                    n.end_line.saturating_sub(n.start_line).saturating_add(1),
                    options.function_lines,
                ),
            ] {
                insert(
                    &mut result,
                    identity.clone(),
                    Metric {
                        code,
                        category,
                        file: n.file_path.clone(),
                        symbol: Some(n.qualified_name.clone()),
                        line: Some(n.start_line.saturating_add(1)),
                        value,
                        threshold,
                        ambiguous: false,
                    },
                );
            }
        }
    }
    let adj = GraphQueryManager::new(cg.db())
        .build_file_adjacency(None)
        .await?;
    let mut incoming: BTreeMap<&str, u32> = adj.keys().map(|k| (k.as_str(), 0)).collect();
    for targets in adj.values() {
        for target in targets {
            *incoming.entry(target).or_default() += 1;
        }
    }
    for (file, targets) in &adj {
        for (code, value) in [
            ("Q_FILE_FAN_OUT", targets.len() as u32),
            (
                "Q_FILE_FAN_IN",
                incoming.get(file.as_str()).copied().unwrap_or(0),
            ),
        ] {
            insert(
                &mut result,
                canonical(file),
                Metric {
                    code,
                    category: "coupling",
                    file: file.clone(),
                    symbol: None,
                    line: None,
                    value,
                    ambiguous: false,
                    threshold: options.fan_out,
                },
            );
        }
    }
    // Report edges within cyclic SCCs, not arbitrary cycle enumerations (which can be exponential).
    for component in scc::tarjan_scc(&adj) {
        if !scc::is_cyclic_scc(&component, &adj) {
            continue;
        }
        let members: BTreeSet<_> = component.iter().collect();
        for file in &component {
            for target in &adj[file] {
                if members.contains(target) {
                    insert(
                        &mut result,
                        serde_json::to_string(&(canonical(file), canonical(target)))
                            .map_err(|e| error(e.to_string()))?,
                        Metric {
                            code: "Q_CYCLIC_DEPENDENCY_EDGE",
                            category: "dependency",
                            file: file.clone(),
                            symbol: Some(target.clone()),
                            line: None,
                            value: 1,
                            ambiguous: false,
                            threshold: 0,
                        },
                    );
                }
            }
        }
    }
    Ok(result)
}

fn classify(base: &Snapshot, target: &Snapshot, min_increase: u32) -> Vec<Finding> {
    let keys: BTreeSet<_> = base.keys().chain(target.keys()).collect();
    keys.into_iter().filter_map(|key| {
        let b = base.get(key); let t = target.get(key);
        let metric = t.or(b)?;
        let bv = b.map(|m| m.value); let tv = t.map(|m| m.value);
        let bad_b = b.is_some_and(|m| m.value > m.threshold);
        let bad_t = t.is_some_and(|m| m.value > m.threshold);
        if !bad_b && !bad_t { return None; }
        let status = match (bad_b, bad_t) {
            (false, true) => "new", (true, false) => "resolved",
            (true, true) if tv > bv && tv.unwrap_or(0).saturating_sub(bv.unwrap_or(0)) >= min_increase.max(1) => "worsened",
            (true, true) if tv < bv => "improved", _ => "unchanged",
        };
        Some(Finding { code: metric.code.into(), category: metric.category.into(), severity: "warning".into(), identity: key.1.clone(), file: metric.file.clone(), symbol: metric.symbol.clone(), line: metric.line, base_value: bv, target_value: tv, status: status.into(), confidence: if b.is_some() && t.is_some() { "indexed" } else { "unmatched" }.into(), explanation: format!("{} exceeds {} when active; values derive from indexed structure. Unmatched symbols may be renamed or moved.", metric.code, metric.threshold) })
    }).collect()
}

/// Compare existing indexes; never sync, checkout, or open a writable database.
pub async fn compare(
    root: &Path,
    base: &str,
    target: &str,
    options: &QualityOptions,
) -> Result<QualityReport> {
    let (base_name, base_commit) = resolve(root, base)?;
    let (target_name, target_commit) = resolve(root, target)?;
    let renamed = git(
        root,
        &[
            "diff",
            "--name-status",
            "--relative",
            "-z",
            "--find-renames",
            &base_commit,
            &target_commit,
            "--",
        ],
    )?;
    let fields: Vec<_> = renamed.split(|b| *b == 0).collect();
    let mut renames = BTreeMap::new();
    let mut i = 0;
    while i + 1 < fields.len() {
        if fields[i].starts_with(b"R") && i + 2 < fields.len() {
            renames.insert(
                String::from_utf8_lossy(fields[i + 1]).into_owned(),
                String::from_utf8_lossy(fields[i + 2]).into_owned(),
            );
            i += 3;
        } else {
            i += 2;
        }
    }
    let base_cg = TokenSave::open_read_only(root, Some(&base_name)).await?;
    base_cg
        .db()
        .conn()
        .execute_batch("BEGIN")
        .await
        .map_err(|e| error(e.to_string()))?;
    let mut stale = verify(&base_cg, &base_commit).await?;
    if !stale.is_empty() && !options.allow_stale {
        return Err(error(format!(
            "base index is stale or incomplete: {}. Sync explicitly, or opt in with --allow-stale (MCP thresholds.allow_stale).",
            stale[0]
        )));
    }
    let base_version = base_cg.db().get_metadata("last_full_index_version").await?;
    let mut base_snapshot = snapshot(&base_cg, options, &renames).await?;
    drop(base_cg);
    // Reuse the pinned metrics for an identical index. A live server may sync
    // between sequential reads; that must never manufacture same-branch changes.
    let mut target_snapshot = if base_name == target_name {
        if base_version.is_none() {
            stale.push("analysis version provenance is missing".into());
            if !options.allow_stale {
                return Err(error(
                    "analysis version provenance is missing; sync the index explicitly",
                ));
            }
        }
        base_snapshot.clone()
    } else {
        let target_cg = TokenSave::open_read_only(root, Some(&target_name)).await?;
        target_cg
            .db()
            .conn()
            .execute_batch("BEGIN")
            .await
            .map_err(|e| error(e.to_string()))?;
        let target_stale = verify(&target_cg, &target_commit).await?;
        if !target_stale.is_empty() && !options.allow_stale {
            return Err(error(format!(
            "target index is stale or incomplete: {}. Sync explicitly, or opt in with --allow-stale (MCP thresholds.allow_stale).",
            target_stale[0]
        )));
        }
        let target_version = target_cg
            .db()
            .get_metadata("last_full_index_version")
            .await?;
        if base_version.is_none() || target_version.is_none() || base_version != target_version {
            stale.push("analysis version provenance is missing or differs between indexes; metrics may not be comparable".into());
            if !options.allow_stale {
                return Err(error("analysis version provenance is missing or differs; explicitly sync both indexes with the same version"));
            }
        }
        stale.extend(target_stale);
        snapshot(&target_cg, options, &BTreeMap::new()).await?
    };
    if resolve(root, base)?.1 != base_commit || resolve(root, target)?.1 != target_commit {
        return Err(error(
            "Git refs changed during comparison; retry with stable branch tips",
        ));
    }
    let ambiguous: BTreeSet<_> = base_snapshot
        .iter()
        .chain(target_snapshot.iter())
        .filter(|(_, metric)| metric.ambiguous)
        .map(|(key, _)| key.clone())
        .collect();
    base_snapshot.retain(|key, _| !ambiguous.contains(key));
    target_snapshot.retain(|key, _| !ambiguous.contains(key));
    let findings = classify(&base_snapshot, &target_snapshot, options.min_increase);
    let mut summary: BTreeMap<String, usize> =
        ["new", "worsened", "resolved", "improved", "unchanged"]
            .into_iter()
            .map(|s| (s.into(), 0))
            .collect();
    for f in &findings {
        *summary.entry(f.status.clone()).or_default() += 1;
    }
    let complete = stale.is_empty() && ambiguous.is_empty();
    let mut limitations = vec![
        "Tip-to-tip comparison, not merge-base attribution: diverged branch differences may originate on either branch.".into(),
        "Only indexed files and resolved calls/uses edges are analyzed. Excluded, unindexed or unsupported source and missing edges are not evidence of clean code.".into(),
        "Git-detected file renames are normalized; symbol renames and ambiguous moves remain unmatched. Unmatched findings require review.".into(),
        "Architecture policy, dead-code, test reachability, execution coverage and module-size regressions are not analyzed.".into(),
        "A small increase below min_increase is classified unchanged for gating. No claim of execution behavior or confirmed defects.".into(),
    ];
    if !ambiguous.is_empty() {
        limitations.push(format!("{} ambiguous metrics (same qualified name, kind and signature) excluded from both snapshots; comparison is incomplete.", ambiguous.len()));
    }
    limitations.extend(stale);
    Ok(QualityReport {
        base: base.into(),
        target: target.into(),
        base_commit,
        target_commit,
        baseline: "branch_tips".into(),
        complete,
        summary,
        findings,
        limitations,
    })
}
