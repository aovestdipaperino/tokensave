//! Consumer semantics of OKF v0.3 shared by the validator and the
//! knowledge search: which version rules apply to a bundle (§12), its
//! bundle-level metadata (§8.1), how absent, empty and single-item values
//! are read (§11.2), lifecycle (§5.4) and attestability (§10).

use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use super::bundle::Bundle;
use super::bundle::BundleFile;
use super::bundle::FieldTarget;
use super::bundle::Role;

/// How a bundle is read, from the `okf_version` its root `index.md`
/// declares (§12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadMode {
    /// The bundle declares OKF 0.3.
    Current,
    /// The bundle declares 0.2 or earlier, or no version: v0.3 rules with
    /// the fallbacks of §12.1 (bundle-root path fields, v0.1 `timestamp`)
    /// and tokensave's earlier readings (`tags` split at commas, `log.md`
    /// frontmatter tolerated).
    Legacy,
    /// The bundle declares a version tokensave does not know: v0.3 rules,
    /// best effort (§12).
    BestEffort,
}

impl ReadMode {
    /// The read mode for a declared `okf_version`.
    pub fn for_version(version: Option<&str>) -> Self {
        let Some(version) = version.map(str::trim) else {
            return Self::Legacy;
        };
        let parsed = version.split_once('.').and_then(|(major, minor)| {
            Some((major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?))
        });
        match parsed {
            Some((0, 0..=2)) => Self::Legacy,
            Some((0, 3)) => Self::Current,
            _ => Self::BestEffort,
        }
    }

    /// Whether the v0.2 and v0.1 fallbacks of §12.1 apply.
    pub fn legacy(self) -> bool {
        self == Self::Legacy
    }
}

/// Bundle-level metadata from the frontmatter of the bundle-root
/// `index.md` (§8.1).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BundleMeta {
    pub okf_version: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// An SPDX expression or a path to a license file in the bundle.
    pub license: Option<String>,
    /// Every other key, kept as written (§4.1, §8.1).
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub extensions: Map<String, Value>,
}

impl BundleMeta {
    /// Reads the metadata of `bundle`. A bundle without a root `index.md`
    /// or without frontmatter in it has none.
    pub fn of(bundle: &Bundle) -> Self {
        let Some(map) = bundle
            .file("index.md")
            .filter(|f| f.role == Role::Index)
            .and_then(BundleFile::frontmatter)
            .and_then(Value::as_object)
        else {
            return Self::default();
        };
        let mut meta = Self::default();
        for (key, value) in map {
            match key.as_str() {
                "okf_version" => meta.okf_version = version_text(value),
                "title" => meta.title = text(value),
                "description" => meta.description = text(value),
                "license" => meta.license = text(value),
                _ => {
                    meta.extensions.insert(key.clone(), value.clone());
                }
            }
        }
        meta
    }

    pub fn read_mode(&self) -> ReadMode {
        ReadMode::for_version(self.okf_version.as_deref())
    }
}

/// `okf_version` as text; an unquoted `0.3` is a YAML number.
pub fn version_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Whether a value counts as absent: null, an empty string or an empty list
/// (§11.2 rule 1).
pub fn is_absent(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

/// The value of `key`, or `None` when it is absent (§11.2 rule 1).
pub fn present<'a>(frontmatter: &'a Value, key: &str) -> Option<&'a Value> {
    frontmatter.get(key).filter(|v| !is_absent(v))
}

/// The items of a list field, accepting one item as a list with one item
/// (§11.2 rule 2).
pub fn items(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

/// A non-empty string field.
pub fn string<'a>(frontmatter: &'a Value, key: &str) -> Option<&'a str> {
    present(frontmatter, key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// `tags` as a list. A scalar is one tag (§11.2 rule 2); only bundles read
/// in [`ReadMode::Legacy`] keep tokensave's earlier reading, which split a
/// scalar at commas.
pub fn tags(frontmatter: &Value, mode: ReadMode) -> Vec<String> {
    let Some(value) = present(frontmatter, "tags") else {
        return Vec::new();
    };
    let scalar = |v: &Value| match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    };
    match value {
        Value::String(s) if mode.legacy() => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        other => items(other)
            .into_iter()
            .filter_map(scalar)
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

/// The lifecycle `status` (§5.4). Absent means `stable`; any other value is
/// kept as written.
pub fn status(frontmatter: &Value) -> String {
    match present(frontmatter, "status") {
        None => "stable".to_string(),
        Some(Value::String(s)) => s.trim().to_string(),
        Some(other) => other.to_string(),
    }
}

/// Whether a concept is current knowledge: its status is `stable` (§5.4).
/// Draft, deprecated and unknown statuses are not.
pub fn is_current(frontmatter: &Value) -> bool {
    status(frontmatter) == "stable"
}

/// The path fields of a concept (§6.2), with the field name and value.
/// `sources[].resource` values that are scope descriptors rather than paths
/// (§5.1) are left out.
pub fn path_fields(frontmatter: &Value) -> Vec<(String, &str)> {
    fn push<'a>(out: &mut Vec<(String, &'a str)>, field: String, value: Option<&'a Value>) {
        if let Some(value) = value.and_then(Value::as_str).map(str::trim) {
            if !value.is_empty() {
                out.push((field, value));
            }
        }
    }
    let mut out = Vec::new();
    push(
        &mut out,
        "resource".into(),
        present(frontmatter, "resource"),
    );
    if let Some(sources) = present(frontmatter, "sources") {
        for (index, source) in items(sources).into_iter().enumerate() {
            let resource = source
                .get("resource")
                .filter(|r| r.as_str().is_some_and(|r| !is_scope_descriptor(r)));
            push(&mut out, format!("sources[{index}].resource"), resource);
        }
    }
    push(
        &mut out,
        "computation".into(),
        present(frontmatter, "computation"),
    );
    for key in ["executor", "attester"] {
        let resource = present(frontmatter, key).and_then(|v| v.get("resource"));
        push(&mut out, format!("{key}.resource"), resource);
    }
    push(
        &mut out,
        "superseded_by".into(),
        present(frontmatter, "superseded_by"),
    );
    out
}

/// A `sources[].resource` that names a population or scope, such as `all
/// queries in BigQuery project X`, rather than a followable artifact (§5.1).
/// Paths and URLs contain no whitespace and have a `/`, `.` or `:`.
pub fn is_scope_descriptor(value: &str) -> bool {
    value.chars().any(char::is_whitespace) || !value.contains(['/', '.', ':'])
}

/// Why an Attested Computation cannot be attested (§10.2, §10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotAttestable {
    NoRuntime,
    NoAttester,
    /// `attester.resource` names a file that is not in the bundle, so the
    /// check cannot run.
    AttesterMissing,
    /// No `computation` file and no fenced block in a `Computation` section.
    NoComputation,
    /// `computation` names a file that is not in the bundle.
    ComputationMissing,
}

/// The `type` of attested computations (§10.1).
pub const ATTESTED_COMPUTATION: &str = "Attested Computation";

/// What keeps a concept of type `Attested Computation` from being attested.
/// An empty list means it is attestable: it has a `runtime`, an `attester`
/// whose file can be found, and a computation that can be found. Consumers
/// must not show a value as attested otherwise (§10.2, §10.3). Requiring the
/// attester file to exist is stricter than §10.2, in the safe direction: a
/// check that cannot run attests nothing.
pub fn attestation_gaps(bundle: &Bundle, file: &BundleFile, mode: ReadMode) -> Vec<NotAttestable> {
    let mut gaps = Vec::new();
    let Some(fm) = file.frontmatter() else {
        return vec![NotAttestable::NoRuntime, NotAttestable::NoAttester];
    };
    if string(fm, "runtime").is_none() {
        gaps.push(NotAttestable::NoRuntime);
    }
    // An external file cannot be read here, but it is declared.
    let resolves = |value: &str| match bundle.resolve_field(&file.path, value, mode.legacy()) {
        FieldTarget::Found { path, .. } => !path.ends_with('/'),
        FieldTarget::External => true,
        FieldTarget::Escapes | FieldTarget::Missing { .. } => false,
    };
    let attester = present(fm, "attester")
        .and_then(|a| a.get("resource"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|r| !r.is_empty());
    match attester {
        None => gaps.push(NotAttestable::NoAttester),
        Some(resource) if !resolves(resource) => gaps.push(NotAttestable::AttesterMissing),
        Some(_) => {}
    }
    match string(fm, "computation") {
        Some(computation) => {
            if !resolves(computation) {
                gaps.push(NotAttestable::ComputationMissing);
            }
        }
        None => {
            if !file
                .outline()
                .is_some_and(|o| o.section_has_fence("Computation"))
            {
                gaps.push(NotAttestable::NoComputation);
            }
        }
    }
    gaps
}

/// Where a deprecated concept's `superseded_by` points (§5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Supersession {
    /// A concept of the bundle, by path.
    Concept(String),
    /// A URL or other value outside the bundle.
    External(String),
    /// A path that names no concept of the bundle.
    Missing(String),
}

/// Resolves the `superseded_by` of `file`, if it has one.
pub fn superseded_by(bundle: &Bundle, file: &BundleFile, mode: ReadMode) -> Option<Supersession> {
    let value = file
        .frontmatter()
        .and_then(|fm| string(fm, "superseded_by"))?;
    Some(
        match bundle.resolve_field(&file.path, value, mode.legacy()) {
            FieldTarget::Found { path, .. } if bundle.is_concept(&path) => {
                Supersession::Concept(path)
            }
            FieldTarget::External | FieldTarget::Escapes => {
                Supersession::External(value.to_string())
            }
            FieldTarget::Found { .. } | FieldTarget::Missing { .. } => {
                Supersession::Missing(value.to_string())
            }
        },
    )
}

/// Most `superseded_by` links followed to find a replacement.
pub const MAX_SUPERSESSION_HOPS: usize = 32;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn read_modes_follow_the_declared_version() {
        assert_eq!(ReadMode::for_version(None), ReadMode::Legacy);
        assert_eq!(ReadMode::for_version(Some("0.1")), ReadMode::Legacy);
        assert_eq!(ReadMode::for_version(Some("0.2")), ReadMode::Legacy);
        assert_eq!(ReadMode::for_version(Some(" 0.3 ")), ReadMode::Current);
        assert_eq!(ReadMode::for_version(Some("0.4")), ReadMode::BestEffort);
        assert_eq!(ReadMode::for_version(Some("1.0")), ReadMode::BestEffort);
        assert_eq!(ReadMode::for_version(Some("v3")), ReadMode::BestEffort);
    }

    #[test]
    fn absent_empty_and_single_values() {
        let fm = json!({"a": null, "b": "", "c": [], "d": "x", "e": {"k": 1}});
        for key in ["a", "b", "c", "missing"] {
            assert!(present(&fm, key).is_none(), "{key}");
        }
        assert_eq!(items(&fm["d"]), [&json!("x")]);
        assert_eq!(items(&fm["e"]).len(), 1);
    }

    #[test]
    fn scalar_tags_are_one_tag_except_in_legacy_bundles() {
        let fm = json!({"tags": "a, b"});
        assert_eq!(tags(&fm, ReadMode::Current), ["a, b"]);
        assert_eq!(tags(&fm, ReadMode::Legacy), ["a", "b"]);
        let fm = json!({"tags": ["x", 2, " y ", null]});
        assert_eq!(tags(&fm, ReadMode::Current), ["x", "2", "y"]);
    }

    #[test]
    fn status_defaults_to_stable_and_keeps_unknown_values() {
        assert!(is_current(&json!({})));
        assert!(is_current(&json!({"status": null})));
        assert!(!is_current(&json!({"status": "deprecated"})));
        assert!(!is_current(&json!({"status": "retired"})));
        assert_eq!(status(&json!({"status": "retired"})), "retired");
    }

    #[test]
    fn scope_descriptors_are_not_paths() {
        assert!(is_scope_descriptor("all queries in project X"));
        assert!(is_scope_descriptor("warehouse"));
        assert!(!is_scope_descriptor("/policies/p.md"));
        assert!(!is_scope_descriptor("https://example.com"));
        assert!(!is_scope_descriptor("bigquery:proj.ds.t"));
    }
}
