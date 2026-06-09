//! PHP / Composer ecosystem parser — `composer.json`.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Patch, Workspace};

const ECOSYSTEM: &str = "php";

pub fn detect(root: &Path) -> bool {
    root.join("composer.json").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let manifest = root.join("composer.json");
    let raw = std::fs::read_to_string(&manifest).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", manifest.display()),
    })?;
    let doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| TokenSaveError::Config {
            message: format!("failed to parse {}: {e}", manifest.display()),
        })?;

    let pkg_name = doc
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("composer-package")
        .to_string();

    let mut deps = Vec::new();
    for (field, kind) in [("require", DepKind::Normal), ("require-dev", DepKind::Dev)] {
        let Some(obj) = doc.get(field).and_then(|v| v.as_object()) else {
            continue;
        };
        for (name, value) in obj {
            // Skip PHP runtime markers like `php`, `ext-mbstring`.
            if name == "php" || name.starts_with("ext-") || name.starts_with("lib-") {
                continue;
            }
            let version = value.as_str().map(str::to_string);
            deps.push(Dep {
                name: name.clone(),
                version,
                features: Vec::new(),
                optional: false,
                local_path: None,
                kind,
            });
        }
    }

    let mut patches = Vec::new();
    if let Some(obj) = doc.get("replace").and_then(|v| v.as_object()) {
        for (name, value) in obj {
            patches.push(Patch {
                source: "composer-replace".to_string(),
                name: name.clone(),
                replacement: value.as_str().unwrap_or("").to_string(),
            });
        }
    }
    if let Some(obj) = doc.get("conflict").and_then(|v| v.as_object()) {
        for (name, value) in obj {
            patches.push(Patch {
                source: "composer-conflict".to_string(),
                name: name.clone(),
                replacement: value.as_str().unwrap_or("").to_string(),
            });
        }
    }

    let member = Member {
        path: "composer.json".to_string(),
        name: pkg_name,
        deps,
    };
    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![member],
        patches,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_composer_json() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("composer.json"),
            r#"{
              "name": "vendor/pkg",
              "require": { "php": "^8.1", "symfony/console": "^7.0" },
              "require-dev": { "phpunit/phpunit": "^10.0" },
              "replace": { "old/pkg": "self.version" }
            }"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "php");
        let m = &ws.members[0];
        assert_eq!(m.name, "vendor/pkg");
        assert!(!m.deps.iter().any(|d| d.name == "php"));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "symfony/console" && d.kind == DepKind::Normal));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "phpunit/phpunit" && d.kind == DepKind::Dev));
        assert_eq!(ws.patches.len(), 1);
        assert_eq!(ws.patches[0].name, "old/pkg");
    }
}
