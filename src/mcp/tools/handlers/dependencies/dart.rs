//! Dart / Flutter ecosystem parser — `pubspec.yaml` (+ `pubspec.lock`).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};
use super::yaml_util::{as_string, hash_entries, node_field, parse_root, yaml_to_string};

const ECOSYSTEM: &str = "dart";

pub fn detect(root: &Path) -> bool {
    root.join("pubspec.yaml").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = root.join("pubspec.yaml");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    let root_doc = parse_root(&raw).ok_or_else(|| TokenSaveError::Config {
        message: format!("failed to parse YAML at {}", path.display()),
    })?;

    let name = as_string(&root_doc, "name").unwrap_or_else(|| "pubspec".to_string());
    let license = as_string(&root_doc, "license");
    let mut deps = Vec::new();
    for (section, kind) in [
        ("dependencies", DepKind::Normal),
        ("dev_dependencies", DepKind::Dev),
        ("dependency_overrides", DepKind::Other("override")),
    ] {
        let Some(block) = node_field(&root_doc, section) else {
            continue;
        };
        for (dep_name, value) in hash_entries(block) {
            // Skip SDK constraint pseudo-deps: `flutter: sdk: flutter`,
            // `flutter_test: sdk: flutter`, etc. The `sdk:` key is a Dart
            // pubspec convention for bundled framework crates.
            if node_field(value, "sdk").is_some() {
                continue;
            }
            // Bare scalar `flutter: any` etc. (rare but valid SDK constraint).
            if matches!(dep_name.as_str(), "flutter" | "dart") && yaml_to_string(value).is_some()
            {
                continue;
            }
            deps.push(build_dep(&dep_name, value, kind));
        }
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "pubspec.yaml".to_string(),
            name,
            license,
            deps,
        }],
        patches: Vec::new(),
    })
}

fn build_dep(name: &str, value: &yaml_rust2::Yaml, kind: DepKind) -> Dep {
    // Scalar = direct version constraint.
    if let Some(v) = yaml_to_string(value) {
        return Dep {
            name: name.to_string(),
            resolved: None,
            version: Some(v),
            features: Vec::new(),
            optional: false,
            local_path: None,
            kind,
        };
    }
    // Table form: { version: "..." } or { git: { url, ref } } or { path: "..." }.
    let version = as_string(value, "version");
    let local_path = as_string(value, "path");
    let git = node_field(value, "git").and_then(|g| {
        if let Some(s) = yaml_to_string(g) {
            Some(s)
        } else {
            as_string(g, "url")
        }
    });
    // For git deps, use the url as `version` to keep one column meaningful.
    let final_version = version.or(git);
    Dep {
        name: name.to_string(),
        resolved: None,
        version: final_version,
        features: Vec::new(),
        optional: matches!(kind, DepKind::Optional),
        local_path,
        kind,
    }
}

/// Apply `pubspec.lock` to fill `resolved` fields on the workspace's deps.
/// Wired into the central `lockfiles::apply_to_workspace` dispatch.
pub fn apply_lockfile(ws: &mut Workspace) {
    let path = ws.root.join("pubspec.lock");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Some(root) = parse_root(&raw) else {
        return;
    };
    let Some(packages) = node_field(&root, "packages") else {
        return;
    };
    let mut versions: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (pkg_name, info) in hash_entries(packages) {
        if let Some(v) = as_string(info, "version") {
            versions.insert(pkg_name, v);
        }
    }
    for m in &mut ws.members {
        for d in &mut m.deps {
            if d.resolved.is_none() {
                if let Some(v) = versions.get(&d.name) {
                    d.resolved = Some(v.clone());
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_pubspec_yaml() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("pubspec.yaml"),
            "name: my_app\nversion: 1.0.0\n\ndependencies:\n  flutter:\n    sdk: flutter\n  http: ^1.2.0\n  provider:\n    version: ^6.0.0\n\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n  build_runner: ^2.4.0\n",
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert_eq!(m.name, "my_app");
        assert!(m.deps.iter().any(|d| d.name == "http" && d.version.as_deref() == Some("^1.2.0")));
        assert!(m.deps.iter().any(|d| d.name == "provider"));
        // SDK markers are filtered out.
        assert!(!m.deps.iter().any(|d| d.name == "flutter"));
        assert!(!m.deps.iter().any(|d| d.name == "flutter_test"));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "build_runner" && d.kind == DepKind::Dev));
    }

    #[test]
    fn pubspec_lock_stamps_resolved() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("pubspec.yaml"),
            "name: my_app\ndependencies:\n  http: ^1.2.0\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("pubspec.lock"),
            "packages:\n  http:\n    dependency: \"direct main\"\n    version: \"1.2.1\"\n",
        )
        .unwrap();
        let mut ws = parse(dir.path()).unwrap();
        apply_lockfile(&mut ws);
        let http = ws.members[0].deps.iter().find(|d| d.name == "http").unwrap();
        assert_eq!(http.resolved.as_deref(), Some("1.2.1"));
    }
}
