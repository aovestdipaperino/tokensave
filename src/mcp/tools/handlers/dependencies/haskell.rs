//! Haskell ecosystem parser — `*.cabal` (custom RFC-822-ish text).
//!
//! `build-depends:` is a comma-separated list of `name (constraint)` specs.
//! `stack.yaml` and `cabal.project` are intentionally deferred (YAML / cabal
//! project syntax respectively).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "haskell";

pub fn detect(root: &Path) -> bool {
    cabal_file(root).is_some()
}

fn cabal_file(root: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut matches: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|s| s.to_str()) == Some("cabal")).then_some(p)
        })
        .collect();
    matches.sort();
    matches.into_iter().next()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = cabal_file(root).ok_or_else(|| TokenSaveError::Config {
        message: format!("no .cabal file found at {}", root.display()),
    })?;
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;

    let pkg_name = field_value(&raw, "name").unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("haskell-package")
            .to_string()
    });

    let mut deps = Vec::new();
    for body in collect_field_bodies(&raw, "build-depends") {
        for spec in body.split(',') {
            if let Some(dep) = parse_dep_spec(spec.trim(), DepKind::Normal) {
                deps.push(dep);
            }
        }
    }

    let manifest_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package.cabal".to_string());

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: manifest_name,
            name: pkg_name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

fn field_value(raw: &str, key: &str) -> Option<String> {
    let key_lc = key.to_ascii_lowercase();
    for line in raw.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case(&key_lc) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// Cabal has folded continuation lines (deeper indentation). Collect every
/// `build-depends:` block's body across the file (executables, libraries,
/// tests can each have one).
fn collect_field_bodies(raw: &str, key: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let key_lc = key.to_ascii_lowercase();
    let mut lines = raw.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if !k.trim().eq_ignore_ascii_case(&key_lc) {
            continue;
        }
        let mut body = v.trim().to_string();
        // Read continuation lines (those starting with more whitespace than
        // the field name).
        while let Some(next) = lines.peek() {
            if next.starts_with(' ') || next.starts_with('\t') {
                body.push(' ');
                body.push_str(next.trim());
                lines.next();
            } else {
                break;
            }
        }
        out.push(body);
    }
    out
}

fn parse_dep_spec(spec: &str, kind: DepKind) -> Option<Dep> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    // Forms: `name`, `name (>= 1.0)`, `name >= 1.0 && < 2.0`, `name == 1.0`.
    let (name, version) = if let Some(i) = spec.find('(') {
        (
            spec[..i].trim().to_string(),
            Some(
                spec[i + 1..]
                    .trim_end_matches(')')
                    .trim()
                    .to_string(),
            ),
        )
    } else {
        let mut idx = spec.len();
        for (i, c) in spec.char_indices() {
            if c.is_ascii_whitespace() && i > 0 {
                idx = i;
                break;
            }
        }
        let n = spec[..idx].trim().to_string();
        let v = spec[idx..].trim();
        let version = (!v.is_empty()).then(|| v.to_string());
        (n, version)
    };
    if name.is_empty() {
        return None;
    }
    Some(Dep {
        name,
        resolved: None,
        version,
        features: Vec::new(),
        optional: false,
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
    fn parses_cabal_build_depends() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("mypkg.cabal"),
            "cabal-version: 2.4
name: mypkg
version: 0.1.0
synopsis: example

library
  build-depends:
      base >= 4.14 && < 5
    , text
    , aeson (>= 2.0)
",
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert_eq!(m.name, "mypkg");
        assert!(m.deps.iter().any(|d| d.name == "base"));
        assert!(m.deps.iter().any(|d| d.name == "text"));
        let aeson = m.deps.iter().find(|d| d.name == "aeson").unwrap();
        assert_eq!(aeson.version.as_deref(), Some(">= 2.0"));
    }
}
