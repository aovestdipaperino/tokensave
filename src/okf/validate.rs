//! OKF v0.2 conformance checking (§11) with actionable diagnostics.
//!
//! Errors are violations of the three conformance rules in §11. Warnings
//! flag SHOULD-level guidance from §5 to §10 that a producer can fix. Info
//! findings are facts the specification says consumers must tolerate, such
//! as broken links, reported so authors can see them.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::bundle::Bundle;
use super::bundle::BundleFile;
use super::bundle::Content;
use super::bundle::Role;
use super::bundle::SkipReason;
use super::bundle::Target;
use super::time;
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
    /// Stable identifier, e.g. `OKF-E003`.
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
    pub concepts: usize,
    pub index_files: usize,
    pub log_files: usize,
    /// False when a reader limit stopped the walk before the whole bundle
    /// was checked.
    pub complete: bool,
    /// True when the bundle meets every §11 conformance rule and was
    /// checked completely.
    pub conformant: bool,
    pub errors: usize,
    pub warnings: usize,
    pub diagnostics: Vec<Diagnostic>,
}

struct Checker {
    now: i64,
    diagnostics: Vec<Diagnostic>,
}

impl Checker {
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
}

/// Validates a bundle that has been read. `now` is the Unix time used for
/// staleness findings.
pub fn validate(bundle: &Bundle, now: i64) -> Report {
    let mut checker = Checker {
        now,
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
        let what = match skipped.reason {
            SkipReason::Symlink => "symbolic link not followed",
            SkipReason::HiddenDirectory => "hidden directory skipped",
            SkipReason::TooLarge => "file exceeds the per-file size limit and was not checked",
            SkipReason::Unreadable => "entry could not be read",
        };
        let severity = match skipped.reason {
            SkipReason::TooLarge | SkipReason::Unreadable => Severity::Warning,
            SkipReason::Symlink | SkipReason::HiddenDirectory => Severity::Info,
        };
        checker.push("OKF-I203", severity, &skipped.path, None, what);
    }
    let mut declared_version = None;
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
                format!("frontmatter is not parseable: {e} (§11 rule 1)"),
            ),
            Content::Parsed { .. } => match file.role {
                Role::Concept => check_concept(&mut checker, bundle, file),
                Role::Index => {
                    if let Some(version) = check_index(&mut checker, file) {
                        declared_version = Some(version);
                    }
                }
                Role::Log => check_log(&mut checker, file),
            },
        }
        check_links(&mut checker, bundle, file);
    }
    check_case_collisions(&mut checker, bundle);
    if let Some(version) = &declared_version {
        if version != SPEC_VERSION {
            checker.push(
                "OKF-I207",
                Severity::Info,
                "index.md",
                None,
                format!(
                    "bundle declares okf_version {version:?}; checked against {SPEC_VERSION} on a \
                     best-effort basis (§12)"
                ),
            );
        }
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
    Report {
        spec_version: SPEC_VERSION,
        declared_version,
        concepts: count(Role::Concept),
        index_files: count(Role::Index),
        log_files: count(Role::Log),
        complete: bundle.complete,
        conformant: errors == 0,
        errors,
        warnings,
        diagnostics,
    }
}

fn check_concept(checker: &mut Checker, bundle: &Bundle, file: &BundleFile) {
    let path = file.path.as_str();
    let Some(fm) = file.frontmatter() else {
        checker.push(
            "OKF-E001",
            Severity::Error,
            path,
            Some(1),
            "concept has no YAML frontmatter block; add one with at least `type:` (§11 rule 1)",
        );
        return;
    };
    let type_name = match fm.get("type") {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.as_str()),
        Some(Value::String(_) | Value::Null) | None => {
            checker.push(
                "OKF-E003",
                Severity::Error,
                path,
                Some(1),
                "frontmatter has no non-empty `type` (§11 rule 2)",
            );
            None
        }
        Some(_) => {
            checker.push(
                "OKF-E003",
                Severity::Error,
                path,
                Some(1),
                "`type` must be a string (§4.1)",
            );
            None
        }
    };
    for key in ["title", "description", "resource"] {
        if let Some(value) = fm.get(key) {
            if !value.is_string() {
                checker.push(
                    "OKF-W109",
                    Severity::Warning,
                    path,
                    Some(1),
                    format!("`{key}` should be a string (§4.1)"),
                );
            }
        }
    }
    if let Some(tags) = fm.get("tags") {
        let ok = tags
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string));
        if !ok {
            checker.push(
                "OKF-W109",
                Severity::Warning,
                path,
                Some(1),
                "`tags` should be a YAML list of strings (§4.1)",
            );
        }
    }
    check_trust(checker, path, fm);
    check_sources(checker, bundle, file, fm);
    if type_name == Some("Attested Computation") {
        check_computation(checker, bundle, file, fm);
    }
}

fn check_timestamp(checker: &mut Checker, path: &str, field: &str, value: &Value) -> Option<i64> {
    let parsed = value.as_str().and_then(time::parse_datetime);
    if parsed.is_none() {
        checker.push(
            "OKF-W102",
            Severity::Warning,
            path,
            Some(1),
            format!(
                "`{field}` should be an ISO 8601 datetime with an explicit UTC offset, e.g. \
                 2026-06-30T14:00:00Z (§5)"
            ),
        );
    }
    parsed
}

fn check_trust(checker: &mut Checker, path: &str, fm: &Value) {
    match fm.get("generated") {
        None => {
            if fm.get("timestamp").is_some() {
                checker.push(
                    "OKF-I205",
                    Severity::Info,
                    path,
                    Some(1),
                    "legacy v0.1 `timestamp` without `generated`; v0.2 records it as \
                     `generated: { by, at }` (§13.1)",
                );
            }
        }
        Some(Value::Object(generated)) => {
            if !generated.get("by").is_some_and(Value::is_string) {
                checker.push(
                    "OKF-W103",
                    Severity::Warning,
                    path,
                    Some(1),
                    "`generated.by` is required within `generated` and should name an actor \
                     (§5.2, §7)",
                );
            }
            if let Some(at) = generated.get("at") {
                check_timestamp(checker, path, "generated.at", at);
            }
        }
        Some(_) => checker.push(
            "OKF-W103",
            Severity::Warning,
            path,
            Some(1),
            "`generated` should be a mapping `{ by, at }` (§5.2)",
        ),
    }
    if let Some(verified) = fm.get("verified") {
        // §5.2: a bare mapping is a one-element list.
        let events: Vec<&Value> = match verified {
            Value::Array(items) => items.iter().collect(),
            other => vec![other],
        };
        for (index, event) in events.into_iter().enumerate() {
            let ok = event.as_object().is_some_and(|e| {
                e.get("by").is_some_and(Value::is_string) && e.get("at").is_some()
            });
            if !ok {
                checker.push(
                    "OKF-W104",
                    Severity::Warning,
                    path,
                    Some(1),
                    format!(
                        "`verified` entry {index} should be a mapping with `by` and `at` (§5.2)"
                    ),
                );
            } else if let Some(at) = event.get("at") {
                check_timestamp(checker, path, &format!("verified[{index}].at"), at);
            }
        }
    }
    if let Some(status) = fm.get("status") {
        if !matches!(status.as_str(), Some("draft" | "stable" | "deprecated")) {
            checker.push(
                "OKF-W101",
                Severity::Warning,
                path,
                Some(1),
                "`status` should be one of draft, stable, deprecated (§5.4)",
            );
        }
    }
    if let Some(stale_after) = fm.get("stale_after") {
        if let Some(instant) = check_timestamp(checker, path, "stale_after", stale_after) {
            if checker.now >= instant {
                checker.push(
                    "OKF-I204",
                    Severity::Info,
                    path,
                    Some(1),
                    format!(
                        "concept is stale: `stale_after` {} has passed (§5.5)",
                        stale_after.as_str().unwrap_or_default()
                    ),
                );
            }
        }
    }
}

fn check_sources(checker: &mut Checker, bundle: &Bundle, file: &BundleFile, fm: &Value) {
    let path = file.path.as_str();
    if let Some(window) = fm.get("usage_window") {
        check_window(checker, path, "usage_window", window);
    }
    let Some(sources) = fm.get("sources") else {
        return;
    };
    let Some(sources) = sources.as_array() else {
        checker.push(
            "OKF-W105",
            Severity::Warning,
            path,
            Some(1),
            "`sources` should be a list of entries (§5.1)",
        );
        return;
    };
    let mut ids = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Some(entry) = source.as_object() else {
            checker.push(
                "OKF-W105",
                Severity::Warning,
                path,
                Some(1),
                format!("`sources[{index}]` should be a mapping (§5.1)"),
            );
            continue;
        };
        match entry.get("resource").and_then(Value::as_str) {
            Some(resource) if !resource.trim().is_empty() => {
                check_path_field(
                    checker,
                    bundle,
                    file,
                    &format!("sources[{index}].resource"),
                    resource,
                );
            }
            _ => checker.push(
                "OKF-W105",
                Severity::Warning,
                path,
                Some(1),
                format!("`sources[{index}]` is missing the required `resource` (§5.1)"),
            ),
        }
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            if ids.contains(&id) {
                checker.push(
                    "OKF-W110",
                    Severity::Warning,
                    path,
                    Some(1),
                    format!("source id {id:?} is used more than once (§5.1)"),
                );
            }
            ids.push(id);
        }
        if let Some(count) = entry.get("usage_count") {
            if count.as_u64().is_none() {
                checker.push(
                    "OKF-W105",
                    Severity::Warning,
                    path,
                    Some(1),
                    format!(
                        "`sources[{index}].usage_count` should be a non-negative integer (§5.1)"
                    ),
                );
            }
        }
        if let Some(modified) = entry.get("last_modified") {
            check_timestamp(
                checker,
                path,
                &format!("sources[{index}].last_modified"),
                modified,
            );
        }
        if let Some(window) = entry.get("usage_window") {
            check_window(
                checker,
                path,
                &format!("sources[{index}].usage_window"),
                window,
            );
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

fn check_window(checker: &mut Checker, path: &str, field: &str, window: &Value) {
    let Some(map) = window.as_object() else {
        checker.push(
            "OKF-W102",
            Severity::Warning,
            path,
            Some(1),
            format!("`{field}` should be a mapping `{{ from, to }}` (§5.1)"),
        );
        return;
    };
    for bound in ["from", "to"] {
        match map.get(bound) {
            Some(value) => {
                check_timestamp(checker, path, &format!("{field}.{bound}"), value);
            }
            None => checker.push(
                "OKF-W102",
                Severity::Warning,
                path,
                Some(1),
                format!("`{field}` should have `{bound}` (§5.1)"),
            ),
        }
    }
}

/// Reports path-valued fields (§6.2) that point at nothing in the bundle.
/// A bare relative path is tried against the document's directory and the
/// bundle root, since published bundles use both readings.
fn check_path_field(
    checker: &mut Checker,
    bundle: &Bundle,
    file: &BundleFile,
    field: &str,
    value: &str,
) {
    // A value with whitespace is a scope descriptor, not a path (§5.1).
    if value.chars().any(char::is_whitespace) || !value.contains(['/', '.']) {
        return;
    }
    let candidates = if value.starts_with('/') || value.starts_with('.') {
        vec![super::bundle::resolve(&file.path, value)]
    } else {
        vec![
            super::bundle::resolve(&file.path, value),
            super::bundle::resolve("", &format!("/{value}")),
        ]
    };
    if candidates.iter().any(|t| match t {
        Target::External => true,
        Target::Internal(p) => bundle.contains(p),
        Target::Escapes => false,
    }) {
        return;
    }
    checker.push(
        "OKF-I201",
        Severity::Info,
        &file.path,
        Some(1),
        format!("`{field}` {value:?} does not resolve to a file in the bundle (§6.2)"),
    );
}

fn check_computation(checker: &mut Checker, bundle: &Bundle, file: &BundleFile, fm: &Value) {
    let path = file.path.as_str();
    let runtime = fm.get("runtime").and_then(Value::as_str);
    if runtime.is_none_or(|r| r.trim().is_empty()) {
        checker.push(
            "OKF-W106",
            Severity::Warning,
            path,
            Some(1),
            "an Attested Computation requires a non-empty `runtime` (§10.2)",
        );
    }
    if let Some(parameters) = fm.get("parameters") {
        let ok = parameters.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|p| p.get("name").is_some_and(Value::is_string))
        });
        if !ok {
            checker.push(
                "OKF-W108",
                Severity::Warning,
                path,
                Some(1),
                "`parameters` should be a list of `{ name, type, required }` entries (§10.2)",
            );
        }
    }
    if let Some(computation) = fm.get("computation").and_then(Value::as_str) {
        check_path_field(checker, bundle, file, "computation", computation);
    } else {
        let has_heading = file.outline().is_some_and(|o| {
            o.headings
                .iter()
                .any(|h| h.level == 1 && h.text.eq_ignore_ascii_case("Computation"))
        });
        if !has_heading {
            checker.push(
                "OKF-W111",
                Severity::Warning,
                path,
                Some(1),
                "an Attested Computation needs a `computation` path or a `# Computation` \
                 section with the computation (§10.3)",
            );
        }
    }
    for key in ["executor", "attester"] {
        if let Some(value) = fm.get(key) {
            match value.get("resource").and_then(Value::as_str) {
                Some(resource) => {
                    check_path_field(checker, bundle, file, &format!("{key}.resource"), resource);
                }
                None => checker.push(
                    "OKF-W108",
                    Severity::Warning,
                    path,
                    Some(1),
                    format!("`{key}` should name a `resource` (§10.2)"),
                ),
            }
        }
    }
}

/// Checks an `index.md` (§8) and returns the declared `okf_version` of the
/// bundle-root index.
fn check_index(checker: &mut Checker, file: &BundleFile) -> Option<String> {
    let fm = file.frontmatter()?;
    let is_root = file.path == "index.md";
    if !is_root {
        checker.push(
            "OKF-E004",
            Severity::Error,
            &file.path,
            Some(1),
            "index files contain no frontmatter outside the bundle root (§8, §11 rule 3)",
        );
        return None;
    }
    let map = fm.as_object()?;
    let extra: Vec<&str> = map
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "okf_version")
        .collect();
    if !extra.is_empty() {
        checker.push(
            "OKF-E004",
            Severity::Error,
            &file.path,
            Some(1),
            format!(
                "the bundle-root index.md may only carry `okf_version` in frontmatter; found {} \
                 (§8, §11 rule 3)",
                extra.join(", ")
            ),
        );
    }
    match map.get("okf_version") {
        Some(Value::String(v)) => Some(v.clone()),
        // `okf_version: 0.2` without quotes is a YAML number.
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(_) => {
            checker.push(
                "OKF-W112",
                Severity::Warning,
                &file.path,
                Some(1),
                "`okf_version` should be a string such as \"0.2\" (§12)",
            );
            None
        }
        None => None,
    }
}

/// Checks a `log.md` (§9): level-2 headings are ISO 8601 dates, newest first.
fn check_log(checker: &mut Checker, file: &BundleFile) {
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
                    "log date heading {:?} must use ISO 8601 YYYY-MM-DD form (§9, §11 rule 3)",
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

fn check_links(checker: &mut Checker, bundle: &Bundle, file: &BundleFile) {
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
                    "link {:?} points outside the bundle root (§6.1)",
                    link.target
                ),
            ),
            Target::Internal(target) => {
                if !bundle.contains(&target) {
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

/// Concept IDs that differ only in letter case collide on case-insensitive
/// file systems, so such a bundle cannot be checked out intact everywhere.
fn check_case_collisions(checker: &mut Checker, bundle: &Bundle) {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for file in &bundle.files {
        let folded = file.path.to_lowercase();
        if let Some(first) = seen.get(&folded) {
            checker.push(
                "OKF-W114",
                Severity::Warning,
                &file.path,
                None,
                format!("path differs from {first:?} only in letter case; one is lost on case-insensitive file systems"),
            );
        } else {
            seen.insert(folded, &file.path);
        }
    }
}
