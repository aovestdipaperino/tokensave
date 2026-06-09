//! R ecosystem parser — `DESCRIPTION` (RFC-822-style key/value).
//!
//! `Depends:`, `Imports:`, `Suggests:`, `LinkingTo:` fields contain
//! comma-separated package specs like `pkg (>= 1.0)`.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "r";

pub fn detect(root: &Path) -> bool {
    root.join("DESCRIPTION").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = root.join("DESCRIPTION");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;

    let fields = parse_rfc822_fields(&raw);
    let pkg_name = fields
        .get("Package")
        .cloned()
        .unwrap_or_else(|| "r-package".to_string());

    let mut deps = Vec::new();
    for (field, kind) in [
        ("Depends", DepKind::Normal),
        ("Imports", DepKind::Normal),
        ("LinkingTo", DepKind::Build),
        ("Suggests", DepKind::Dev),
        ("Enhances", DepKind::Optional),
    ] {
        if let Some(body) = fields.get(field) {
            for spec in body.split(',') {
                if let Some(dep) = parse_dep_spec(spec.trim(), kind) {
                    deps.push(dep);
                }
            }
        }
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "DESCRIPTION".to_string(),
            name: pkg_name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

/// RFC-822 has folded continuation lines (leading whitespace). Collapse them
/// before splitting on `,`.
fn parse_rfc822_fields(raw: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();
    let flush = |key: &mut Option<String>,
                 val: &mut String,
                 out: &mut std::collections::BTreeMap<String, String>| {
        if let Some(k) = key.take() {
            out.insert(k, val.trim().to_string());
            val.clear();
        }
    };
    for line in raw.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            current_val.push(' ');
            current_val.push_str(line.trim());
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            flush(&mut current_key, &mut current_val, &mut out);
            current_key = Some(k.trim().to_string());
            current_val = v.trim().to_string();
        }
    }
    flush(&mut current_key, &mut current_val, &mut out);
    out
}

fn parse_dep_spec(spec: &str, kind: DepKind) -> Option<Dep> {
    if spec.is_empty() {
        return None;
    }
    // `pkg (>= 1.0)` form.
    let (name, version) = match spec.find('(') {
        Some(i) => {
            let n = spec[..i].trim().to_string();
            let v = spec[i + 1..]
                .trim_end_matches(')')
                .trim()
                .to_string();
            let v_opt = (!v.is_empty()).then_some(v);
            (n, v_opt)
        }
        None => (spec.trim().to_string(), None),
    };
    // R itself is a runtime version marker, not a CRAN dep — filter it out.
    if name.is_empty() || name == "R" {
        return None;
    }
    Some(Dep {
        name,
        resolved: None,
        version,
        features: Vec::new(),
        optional: matches!(kind, DepKind::Optional),
        local_path: None,
        kind,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_description() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("DESCRIPTION"),
            "Package: mypkg
Type: Package
Version: 0.1.0
Depends:
    R (>= 4.0),
    dplyr (>= 1.0.0)
Imports:
    rlang,
    tibble (>= 3.0)
Suggests:
    testthat (>= 3.0.0)
",
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert_eq!(m.name, "mypkg");
        assert!(m.deps.iter().any(|d| d.name == "dplyr" && d.kind == DepKind::Normal));
        assert!(m.deps.iter().any(|d| d.name == "rlang"));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "testthat" && d.kind == DepKind::Dev));
        // R itself is filtered out.
        assert!(!m.deps.iter().any(|d| d.name == "R"));
    }
}
