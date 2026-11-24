//! OKF v0.3 conformance checking (§11) with actionable diagnostics.
//!
//! Errors are violations of the two conformance rules of §11.1: frontmatter
//! that does not parse as a YAML mapping, and reserved files (`index.md`,
//! `log.md`) that do not follow §8 and §9. Warnings flag guidance from §3 to
//! §10 that a producer can fix. Info findings are facts the specification
//! says consumers must tolerate, such as broken links, reported so authors
//! can see them.
//!
//! A bundle that declares `okf_version` 0.2 or earlier, or none, is read
//! with the fallbacks of §12.1; see [`ReadMode`].

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::bundle::Bundle;
use super::bundle::BundleFile;
use super::bundle::Content;
use super::bundle::FieldTarget;
use super::bundle::Role;
use super::bundle::SkipReason;
use super::bundle::Target;
use super::consumer;
use super::consumer::items;
use super::consumer::present;
use super::consumer::BundleMeta;
use super::consumer::NotAttestable;
use super::consumer::ReadMode;
use super::consumer::Supersession;
use super::time;
use super::trust;
use super::SPEC_VERSION;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        })
    }
}

/// One validation finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable identifier, e.g. `OKF-E002`.
    pub code: &'static str,
    pub severity: Severity,
    /// Bundle-relative path, or empty for the bundle as a whole.
    pub path: String,
    /// 1-based line, when the finding has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub message: String,
}

/// The outcome of validating a bundle.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The OKF version the checks implement.
    pub spec_version: &'static str,
    /// `okf_version` declared by the bundle-root `index.md`, if any.
    pub declared_version: Option<String>,
    /// Which version rules were applied (§12).
    pub read_mode: ReadMode,
    /// Bundle metadata from the bundle-root `index.md` (§8.1).
    pub metadata: BundleMeta,
    /// Markdown files that are concepts (§3.2).
    pub concepts: usize,
    /// Other markdown files that are not reserved: no frontmatter or no
    /// `type`. Consumers skip them.
    pub non_concepts: usize,
    pub index_files: usize,
    pub log_files: usize,
    /// False when a reader limit stopped the walk before the whole bundle
    /// was checked.
    pub complete: bool,
    /// True when the bundle meets every §11.1 conformance rule and was
    /// checked completely.
    pub conformant: bool,
    pub errors: usize,
    pub warnings: usize,
    pub diagnostics: Vec<Diagnostic>,
}

struct Checker<'a> {
    bundle: &'a Bundle,
    now: i64,
    mode: ReadMode,
    diagnostics: Vec<Diagnostic>,
}

impl Checker<'_> {
    fn push(
        &mut self,
        code: &'static str,
        severity: Severity,
        path: &str,
        line: Option<usize>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            code,
            severity,
            path: path.to_string(),
            line,
            message: message.into(),
        });
    }

    fn warn(&mut self, code: &'static str, path: &str, message: impl Into<String>) {
        self.push(code, Severity::Warning, path, Some(1), message);
    }

    fn info(&mut self, code: &'static str, path: &str, message: impl Into<String>) {
        self.push(code, Severity::Info, path, Some(1), message);
    }
}

/// Validates a bundle that has been read. `now` is the Unix time used for
/// staleness findings.
pub fn validate(bundle: &Bundle, now: i64) -> Report {
    let metadata = BundleMeta::of(bundle);
    let mode = metadata.read_mode();
    let mut checker = Checker {
        bundle,
        now,
        mode,
        diagnostics: Vec::new(),
    };
    if !bundle.complete {
        checker.push(
            "OKF-E000",
            Severity::Error,
            "",
            None,
            "validation stopped at a size limit; the bundle was not fully checked",
        );
    }
    for skipped in &bundle.skipped {
        let (severity, what) = match skipped.reason {
            SkipReason::Symlink => (Severity::Info, "symbolic link not followed (§3.2)"),
            SkipReason::HiddenDirectory => (
                Severity::Info,
                "hidden directory is not part of the bundle (§3.2)",
            ),
            SkipReason::HiddenFile => (
                Severity::Info,
                "hidden file is not part of the bundle (§3.2)",
            ),
            SkipReason::TooLarge => (
                Severity::Warning,
                "file exceeds the per-file size limit and was not checked",
            ),
            SkipReason::Unreadable => (Severity::Warning, "entry could not be read"),
        };
        checker.push("OKF-I203", severity, &skipped.path, None, what);
    }
    for file in &bundle.files {
        match &file.content {
            Content::NotUtf8 => checker.push(
                "OKF-E006",
                Severity::Error,
                &file.path,
                None,
                "file is not valid UTF-8 (§4)",
            ),
            Content::BadFrontmatter(e) => checker.push(
                "OKF-E002",
                Severity::Error,
                &file.path,
                Some(1),
                format!("frontmatter is not a parseable YAML mapping: {e} (§4, §11.1 rule 1)"),
            ),
            Content::Parsed { document, .. } => {
                if !document.custom_tags.is_empty() {
                    checker.warn(
                        "OKF-W124",
                        &file.path,
                        format!(
                            "frontmatter uses custom YAML tags ({}); producers should not (§4)",
                            document.custom_tags.join(", ")
                        ),
                    );
                }
                match file.role {
                    Role::Concept => check_markdown(&mut checker, file),
                    Role::Index => check_index(&mut checker, file),
                    Role::Log => check_log(&mut checker, file),
                }
            }
        }
        check_links(&mut checker, file);
    }
    check_case_collisions(&mut checker);
    check_supersession_cycles(&mut checker);
    match &metadata.okf_version {
        Some(version) if version != SPEC_VERSION => {
            let how = if mode.legacy() {
                "read with the v0.2 fallbacks of §12.1"
            } else {
                "an unknown version, read best-effort (§12)"
            };
            checker.info(
                "OKF-I207",
                "index.md",
                format!("bundle declares okf_version {version:?}: {how}; checked against OKF {SPEC_VERSION}"),
            );
        }
        Some(_) => {}
        None => checker.push(
            "OKF-I210",
            Severity::Info,
            "",
            None,
            "bundle declares no okf_version, so it is read with the v0.2 fallbacks of §12.1; \
             declare it in the frontmatter of the bundle-root index.md (§8.1)",
        ),
    }

    let mut diagnostics = checker.diagnostics;
    diagnostics.sort_by(|a, b| {
        (a.severity, &a.path, a.line, a.code).cmp(&(b.severity, &b.path, b.line, b.code))
    });
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let count = |role| bundle.files.iter().filter(|f| f.role == role).count();
    let concepts = bundle
        .files
        .iter()
        .filter(|f| f.concept_type().is_some())
        .count();
    Report {
        spec_version: SPEC_VERSION,
        declared_version: metadata.okf_version.clone(),
        read_mode: mode,
        metadata,
        concepts,
        non_concepts: count(Role::Concept) - concepts,
        index_files: count(Role::Index),
        log_files: count(Role::Log),
        complete: bundle.complete,
        conformant: errors == 0,
        errors,
        warnings,
        diagnostics,
    }
}

/// Checks a markdown file that is not reserved. Only files with a
/// non-empty `type` are concepts (§3.2); others are skipped with a warning.
fn check_markdown(checker: &mut Checker<'_>, file: &BundleFile) {
    let path = file.path.as_str();
    let Some(fm) = file.frontmatter() else {
        checker.warn(
            "OKF-W117",
            path,
            "file has no frontmatter, so it is not a concept and consumers skip it; add \
             frontmatter with `type:` if it should be one (§3.2)",
        );
        return;
    };
    if file.concept_type().is_none() {
        let message = match fm.get("type") {
            Some(value) if !consumer::is_absent(value) && !value.is_string() => {
                "`type` is not a string, so the file is not a concept (§3.2, §4.1)"
            }
            _ => {
                "frontmatter has no non-empty `type`, so the file is not a concept and \
                  consumers skip it (§3.2)"
            }
        };
        checker.warn("OKF-W118", path, message);
        return;
    }
    for key in ["title", "description", "resource"] {
        if present(fm, key).is_some_and(|v| !v.is_string()) {
            checker.warn(
                "OKF-W109",
                path,
                format!("`{key}` should be a string (§4.1)"),
            );
        }
    }
    match present(fm, "tags") {
        Some(Value::String(tag)) if tag.contains(',') => checker.warn(
            "OKF-W109",
            path,
            "`tags` is one string with commas; a scalar is one tag and consumers do not split \
             it, so write a YAML list (§4.1, §11.2)",
        ),
        Some(Value::String(_)) | None => {}
        Some(Value::Array(tags)) if tags.iter().all(Value::is_string) => {}
        Some(_) => checker.warn(
            "OKF-W109",
            path,
            "`tags` should be a YAML list of strings (§4.1)",
        ),
    }
    check_trust(checker, path, fm);
    check_sources(checker, file, fm);
    for (field, value) in consumer::path_fields(fm) {
        if field != "computation" && field != "superseded_by" {
            check_path_field(checker, file, &field, value, true);
        }
    }
    check_lifecycle(checker, file, fm);
    if file.concept_type() == Some(consumer::ATTESTED_COMPUTATION) {
        check_computation(checker, file, fm);
    }
}

fn check_timestamp(
    checker: &mut Checker<'_>,
    path: &str,
    field: &str,
    value: &Value,
) -> Option<i64> {
    let parsed = trust::timestamp(value);
    if parsed.is_none() {
        checker.warn(
            "OKF-W102",
            path,
            format!(
                "`{field}` should be an RFC 3339 date-time with an offset, such as \
                 2026-06-30T14:00:00Z, or a date such as 2026-06-30 (§5)"
            ),
        );
    }
    parsed
}

fn check_actor(checker: &mut Checker<'_>, path: &str, field: &str, actor: &str) {
    if !trust::is_actor(actor) {
        checker.warn(
            "OKF-W119",
            path,
            format!(
                "`{field}` {actor:?} should be an actor: `<producer>/<version>` or \
                 `<scheme>:<id>`, such as human:<id> (§7)"
            ),
        );
    }
}

fn check_trust(checker: &mut Checker<'_>, path: &str, fm: &Value) {
    match present(fm, "generated") {
        None => {
            if present(fm, "timestamp").is_some() {
                let how = if checker.mode.legacy() {
                    "read as `generated.at`"
                } else {
                    "not read in a v0.3 bundle"
                };
                checker.info(
                    "OKF-I205",
                    path,
                    format!("legacy v0.1 `timestamp` without `generated` ({how}); write `generated: {{ by, at }}` (Appendix B)"),
                );
            }
        }
        Some(Value::Object(generated)) => {
            match generated.get("by").and_then(Value::as_str) {
                Some(by) if !by.trim().is_empty() => check_actor(checker, path, "generated.by", by),
                _ => checker.warn(
                    "OKF-W103",
                    path,
                    "`generated.by` is required within `generated` and names an actor (§5.2, §7)",
                ),
            }
            if let Some(at) = generated.get("at").filter(|v| !consumer::is_absent(v)) {
                check_timestamp(checker, path, "generated.at", at);
            }
        }
        Some(_) => checker.warn(
            "OKF-W103",
            path,
            "`generated` should be a mapping `{ by, at }` (§5.2)",
        ),
    }
    if let Some(verified) = present(fm, "verified") {
        // §5.2: a bare mapping is a one-element list.
        for (index, event) in items(verified).into_iter().enumerate() {
            let by = event.get("by").and_then(Value::as_str);
            let at = event.get("at").filter(|v| !consumer::is_absent(v));
            match (by, at) {
                (Some(by), Some(at)) if !by.trim().is_empty() => {
                    check_actor(checker, path, &format!("verified[{index}].by"), by);
                    check_timestamp(checker, path, &format!("verified[{index}].at"), at);
                }
                _ => checker.warn(
                    "OKF-W104",
                    path,
                    format!(
                        "`verified` entry {index} needs both `by` and `at`; consumers ignore it \
                         (§5.2)"
                    ),
                ),
            }
        }
    }
    let assessed = trust::assess(fm, checker.mode);
    if !assessed.earlier.is_empty() {
        checker.info(
            "OKF-I208",
            path,
            format!(
                "{} verification(s) predate the last change (`generated.at`) and are history, \
                 not approval of the current content; trust tier: {} (§5.3)",
                assessed.earlier.len(),
                assessed.tier
            ),
        );
    }
    if let Some(status) = present(fm, "status") {
        if !matches!(status.as_str(), Some("draft" | "stable" | "deprecated")) {
            checker.warn(
                "OKF-W101",
                path,
                "`status` should be one of draft, stable, deprecated; other values are not \
                 current knowledge (§5.4)",
            );
        }
    }
    if let Some(stale_after) = present(fm, "stale_after") {
        match check_timestamp(checker, path, "stale_after", stale_after) {
            Some(instant) if checker.now >= instant => checker.info(
                "OKF-I204",
                path,
                format!(
                    "concept is stale: `stale_after` {} has passed; check it again (§5.5)",
                    stale_after.as_str().unwrap_or_default()
                ),
            ),
            // W102 already says it cannot be read; consumers never treat it
            // as fresh (§5.5).
            Some(_) | None => {}
        }
    }
}

fn check_sources(checker: &mut Checker<'_>, file: &BundleFile, fm: &Value) {
    let path = file.path.as_str();
    if let Some(window) = present(fm, "usage_window") {
        check_window(checker, path, "usage_window", window);
    }
    let mut ids = Vec::new();
    if let Some(sources) = present(fm, "sources") {
        for (index, source) in items(sources).into_iter().enumerate() {
            let Some(entry) = source.as_object() else {
                checker.warn(
                    "OKF-W105",
                    path,
                    format!("`sources[{index}]` should be a mapping (§5.1)"),
                );
                continue;
            };
            let has_resource = entry
                .get("resource")
                .and_then(Value::as_str)
                .is_some_and(|r| !r.trim().is_empty());
            if !has_resource {
                checker.warn(
                    "OKF-W105",
                    path,
                    format!(
                        "`sources[{index}]` is missing the required `resource`; consumers ignore \
                         it (§5.1)"
                    ),
                );
            }
            if let Some(id) = entry.get("id").and_then(Value::as_str) {
                if ids.contains(&id) {
                    checker.warn(
                        "OKF-W110",
                        path,
                        format!("source id {id:?} is used more than once (§5.1)"),
                    );
                }
                ids.push(id);
            }
            if let Some(author) = entry.get("author").and_then(Value::as_str) {
                check_actor(checker, path, &format!("sources[{index}].author"), author);
            }
            if let Some(count) = entry.get("usage_count").filter(|v| !consumer::is_absent(v)) {
                if count.as_u64().is_none() {
                    checker.warn(
                        "OKF-W105",
                        path,
                        format!(
                            "`sources[{index}].usage_count` should be a non-negative integer \
                             (§5.1)"
                        ),
                    );
                }
            }
            if let Some(modified) = entry
                .get("last_modified")
                .filter(|v| !consumer::is_absent(v))
            {
                check_timestamp(
                    checker,
                    path,
                    &format!("sources[{index}].last_modified"),
                    modified,
                );
            }
            if let Some(window) = entry
                .get("usage_window")
                .filter(|v| !consumer::is_absent(v))
            {
                check_window(
                    checker,
                    path,
                    &format!("sources[{index}].usage_window"),
                    window,
                );
            }
        }
    }
    if let Some(outline) = file.outline() {
        for label in &outline.footnotes {
            if !ids.contains(&label.as_str()) {
                checker.push(
                    "OKF-I206",
                    Severity::Info,
                    path,
                    None,
                    format!("footnote [^{label}] does not match any `sources[].id` (§5.1)"),
                );
            }
        }
    }
}

fn check_window(checker: &mut Checker<'_>, path: &str, field: &str, window: &Value) {
    let Some(map) = window.as_object() else {
        checker.warn(
            "OKF-W102",
            path,
            format!("`{field}` should be a mapping `{{ from, to }}` (§5.1)"),
        );
        return;
    };
    for bound in ["from", "to"] {
        match map.get(bound).filter(|v| !consumer::is_absent(v)) {
            Some(value) => {
                check_timestamp(checker, path, &format!("{field}.{bound}"), value);
            }
            None => checker.warn(
                "OKF-W102",
                path,
                format!("`{field}` should have `{bound}` (§5.1)"),
            ),
        }
    }
}

/// Resolves a path field (§6.2) and reports how: a bare path that resolves
/// only from the bundle root is a v0.2 reading, accepted for legacy bundles
/// (`OKF-I211`) and a defect in v0.3 bundles (`OKF-W123`). With
/// `report_missing`, a path that names nothing is reported as `OKF-I201`.
fn check_path_field(
    checker: &mut Checker<'_>,
    file: &BundleFile,
    field: &str,
    value: &str,
    report_missing: bool,
) -> FieldTarget {
    let target = checker
        .bundle
        .resolve_field(&file.path, value, checker.mode.legacy());
    match &target {
        FieldTarget::Found {
            path,
            root_fallback: true,
        } => checker.info(
            "OKF-I211",
            &file.path,
            format!(
                "`{field}` {value:?} resolves only from the bundle root, read with the v0.2 \
                 fallback (§6.2, §12.1); write \"/{path}\" before declaring okf_version 0.3"
            ),
        ),
        FieldTarget::Missing {
            root_only: Some(root),
            ..
        } => checker.warn(
            "OKF-W123",
            &file.path,
            format!(
                "`{field}` {value:?} does not resolve from the file's directory; OKF 0.3 reads \
                 paths without a leading `/` there, not from the bundle root (§6.2). Write \
                 \"/{root}\""
            ),
        ),
        FieldTarget::Missing {
            root_only: None, ..
        } if report_missing => checker.info(
            "OKF-I201",
            &file.path,
            format!("`{field}` {value:?} does not resolve to a file in the bundle (§6.2)"),
        ),
        FieldTarget::Escapes => checker.info(
            "OKF-I202",
            &file.path,
            format!(
                "`{field}` {value:?} points outside the bundle root, so it is external and not \
                 read (§6)"
            ),
        ),
        FieldTarget::Found { .. } | FieldTarget::Missing { .. } | FieldTarget::External => {}
    }
    target
}

/// `status` and `superseded_by` (§5.4).
fn check_lifecycle(checker: &mut Checker<'_>, file: &BundleFile, fm: &Value) {
    let path = file.path.as_str();
    let Some(value) = present(fm, "superseded_by") else {
        return;
    };
    let Some(value) = value.as_str() else {
        checker.warn(
            "OKF-W121",
            path,
            "`superseded_by` should be one path to the replacing concept (§5.4)",
        );
        return;
    };
    if consumer::status(fm) != "deprecated" {
        checker.warn(
            "OKF-W120",
            path,
            "`superseded_by` has a meaning only with `status: deprecated` (§5.4)",
        );
    }
    match check_path_field(checker, file, "superseded_by", value, false) {
        FieldTarget::Found { path: target, .. } if !checker.bundle.is_concept(&target) => {
            checker.warn(
                "OKF-W121",
                path,
                format!("`superseded_by` {value:?} is not a concept of the bundle (§5.4)"),
            );
        }
        FieldTarget::Missing {
            root_only: None, ..
        } => checker.warn(
            "OKF-W121",
            path,
            format!(
                "`superseded_by` {value:?} names nothing in the bundle, so consumers cannot \
                 follow it to the replacement (§5.4)"
            ),
        ),
        _ => {}
    }
}

/// Deprecated concepts whose `superseded_by` chain leads back to themselves.
fn check_supersession_cycles(checker: &mut Checker<'_>) {
    let mut next: BTreeMap<&str, String> = BTreeMap::new();
    for file in &checker.bundle.files {
        let deprecated = file
            .frontmatter()
            .is_some_and(|fm| consumer::status(fm) == "deprecated");
        if !deprecated || file.concept_type().is_none() {
            continue;
        }
        if let Some(Supersession::Concept(target)) =
            consumer::superseded_by(checker.bundle, file, checker.mode)
        {
            next.insert(file.path.as_str(), target);
        }
    }
    let mut cyclic = Vec::new();
    for start in next.keys() {
        let mut current = next.get(start);
        for _ in 0..=next.len() {
            match current {
                Some(path) if path == start => {
                    cyclic.push((*start).to_string());
                    break;
                }
                Some(path) => current = next.get(path.as_str()),
                None => break,
            }
        }
    }
    for path in cyclic {
        checker.warn(
            "OKF-W122",
            &path,
            "`superseded_by` leads back to this concept, so no replacement can be found (§5.4)",
        );
    }
}

fn check_computation(checker: &mut Checker<'_>, file: &BundleFile, fm: &Value) {
    let path = file.path.as_str();
    if consumer::string(fm, "runtime").is_none() {
        checker.warn(
            "OKF-W106",
            path,
            "an Attested Computation requires a non-empty `runtime` (§10.2)",
        );
    }
    if let Some(parameters) = present(fm, "parameters") {
        let ok = items(parameters).iter().all(|p| {
            p.get("name").is_some_and(Value::is_string)
                && p.get("type").is_some_and(Value::is_string)
                && p.get("required")
                    .is_none_or(|r| r.is_boolean() || r.is_null())
        });
        if !ok {
            checker.warn(
                "OKF-W108",
                path,
                "`parameters` entries need a string `name` and `type`, and an optional boolean \
                 `required` (§10.2)",
            );
        }
    }
    if let Some(computation) = present(fm, "computation") {
        match computation.as_str() {
            Some(value) => {
                check_path_field(checker, file, "computation", value, false);
            }
            None => checker.warn("OKF-W108", path, "`computation` should be a path (§10.2)"),
        }
    }
    for key in ["executor", "attester"] {
        let Some(value) = present(fm, key) else {
            continue;
        };
        let has_resource = value
            .get("resource")
            .and_then(Value::as_str)
            .is_some_and(|r| !r.trim().is_empty());
        if !has_resource {
            checker.warn(
                "OKF-W108",
                path,
                format!("`{key}` should be a mapping with a `resource` path (§10.2)"),
            );
        }
    }
    let receipt_ok = present(fm, "executor")
        .and_then(|e| e.get("receipt"))
        .is_none_or(|r| r.is_null() || items(r).iter().all(|f| f.is_string()));
    if !receipt_ok {
        checker.warn(
            "OKF-W108",
            path,
            "`executor.receipt` should be a list of field names (§10.2)",
        );
    }
    let gaps = consumer::attestation_gaps(checker.bundle, file, checker.mode);
    if gaps.contains(&NotAttestable::NoComputation) {
        checker.warn(
            "OKF-W111",
            path,
            "an Attested Computation needs a `computation` path or a fenced code block in a \
             `Computation` section (§10.3)",
        );
    }
    if gaps.contains(&NotAttestable::ComputationMissing) {
        checker.warn(
            "OKF-W111",
            path,
            "`computation` names no file in the bundle, so the computation is missing (§10.3)",
        );
    }
    if !gaps.is_empty() {
        let reasons: Vec<&str> = gaps
            .iter()
            .map(|gap| match gap {
                NotAttestable::NoRuntime => "no runtime",
                NotAttestable::NoAttester => "no attester",
                NotAttestable::AttesterMissing => "attester file not in the bundle",
                NotAttestable::NoComputation | NotAttestable::ComputationMissing => {
                    "no computation"
                }
            })
            .collect();
        checker.info(
            "OKF-I209",
            path,
            format!(
                "not attestable ({}); consumers must not show its values as attested (§10.2)",
                reasons.join(", ")
            ),
        );
    }
}

/// Checks an `index.md` (§8): only the bundle-root index may have
/// frontmatter, with the bundle metadata of §8.1.
fn check_index(checker: &mut Checker<'_>, file: &BundleFile) {
    let Some(fm) = file.frontmatter() else {
        return;
    };
    if file.path != "index.md" {
        checker.push(
            "OKF-E004",
            Severity::Error,
            &file.path,
            Some(1),
            "an index.md outside the bundle root has no frontmatter (§8, §11.1 rule 2)",
        );
        return;
    }
    match present(fm, "okf_version") {
        None | Some(Value::String(_)) => {}
        Some(Value::Number(_)) => checker.warn(
            "OKF-W112",
            &file.path,
            "`okf_version` should be a quoted string such as \"0.3\"; unquoted it is a YAML \
             number (§4, §8.1)",
        ),
        Some(_) => checker.warn(
            "OKF-W112",
            &file.path,
            "`okf_version` should be a string such as \"0.3\" (§8.1)",
        ),
    }
    for key in ["title", "description", "license"] {
        if present(fm, key).is_some_and(|v| !v.is_string()) {
            checker.warn(
                "OKF-W115",
                &file.path,
                format!("bundle metadata `{key}` should be a string (§8.1)"),
            );
        }
    }
    // `license` is an SPDX expression or a path to a license file (§8.1).
    // SPDX expressions contain no `/`.
    if let Some(license) = consumer::string(fm, "license").filter(|l| l.contains('/')) {
        check_path_field(checker, file, "license", license, true);
    }
}

/// Checks a `log.md` (§9): no frontmatter, level-2 headings are ISO 8601
/// dates, newest first.
fn check_log(checker: &mut Checker<'_>, file: &BundleFile) {
    if file.frontmatter().is_some() {
        if checker.mode.legacy() {
            checker.warn(
                "OKF-W116",
                &file.path,
                "log.md has frontmatter; OKF 0.3 log files have none (§9). Tolerated because \
                 the bundle is read with the v0.2 fallbacks",
            );
        } else {
            checker.push(
                "OKF-E007",
                Severity::Error,
                &file.path,
                Some(1),
                "log.md has no frontmatter (§9, §11.1 rule 2)",
            );
        }
    }
    let Some(outline) = file.outline() else {
        return;
    };
    let mut previous: Option<&str> = None;
    for heading in outline.headings.iter().filter(|h| h.level == 2) {
        if !time::is_date(&heading.text) {
            checker.push(
                "OKF-E005",
                Severity::Error,
                &file.path,
                Some(heading.line),
                format!(
                    "log date heading {:?} must use ISO 8601 YYYY-MM-DD form (§9, §11.1 rule 2)",
                    heading.text
                ),
            );
            continue;
        }
        if previous.is_some_and(|p| p < heading.text.as_str()) {
            checker.push(
                "OKF-W113",
                Severity::Warning,
                &file.path,
                Some(heading.line),
                "log entries should be ordered newest first (§9)",
            );
        }
        previous = Some(&heading.text);
    }
}

fn check_links(checker: &mut Checker<'_>, file: &BundleFile) {
    let Some(outline) = file.outline() else {
        return;
    };
    for link in &outline.links {
        match super::bundle::resolve(&file.path, &link.target) {
            Target::External => {}
            Target::Escapes => checker.push(
                "OKF-I202",
                Severity::Info,
                &file.path,
                Some(link.line),
                format!(
                    "link {:?} points outside the bundle root, so it is external and not read \
                     (§6)",
                    link.target
                ),
            ),
            Target::Internal(target) => {
                if !checker.bundle.contains(&target) {
                    checker.push(
                        "OKF-I201",
                        Severity::Info,
                        &file.path,
                        Some(link.line),
                        format!(
                            "link {:?} has no target in the bundle; consumers tolerate this \
                             (§6.1)",
                            link.target
                        ),
                    );
                }
            }
        }
    }
}

/// Paths that differ only in letter case collide on case-insensitive file
/// systems, so such a bundle cannot be checked out intact everywhere (§6).
fn check_case_collisions(checker: &mut Checker<'_>) {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    let mut found = Vec::new();
    for file in &checker.bundle.files {
        let folded = file.path.to_lowercase();
        if let Some(first) = seen.get(&folded) {
            found.push((file.path.clone(), (*first).to_string()));
        } else {
            seen.insert(folded, &file.path);
        }
    }
    for (path, first) in found {
        checker.push(
            "OKF-W114",
            Severity::Warning,
            &path,
            None,
            format!(
                "path differs from {first:?} only in letter case; producers should not do this, \
                 and one is lost on case-insensitive file systems (§6)"
            ),
        );
    }
}
