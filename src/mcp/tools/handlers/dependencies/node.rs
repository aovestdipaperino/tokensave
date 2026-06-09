//! Node.js / npm ecosystem parser — `package.json` (+ workspaces field).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{expand_workspace_globs, Dep, DepKind, Member, Patch, Workspace};

const ECOSYSTEM: &str = "node";

pub fn detect(root: &Path) -> bool {
    root.join("package.json").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let manifest = root.join("package.json");
    let raw = std::fs::read_to_string(&manifest).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", manifest.display()),
    })?;
    let doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| TokenSaveError::Config {
            message: format!("failed to parse {}: {e}", manifest.display()),
        })?;

    let mut members: Vec<Member> = Vec::new();
    if doc.get("name").is_some() || doc.get("private").is_some() {
        members.push(member_from_doc(".", &doc));
    }

    // `workspaces` is either `string[]` or `{ packages: string[] }`.
    let ws_patterns: Vec<String> = match doc.get("workspaces") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        Some(serde_json::Value::Object(obj)) => obj
            .get("packages")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    for member_path in expand_workspace_globs(root, &ws_patterns, "package.json") {
        if let Some(m) = read_member(root, &member_path) {
            members.push(m);
        }
    }

    // `overrides` / `resolutions` act as ecosystem-specific "patches".
    let mut patches: Vec<Patch> = Vec::new();
    for (field, source) in [("overrides", "npm-overrides"), ("resolutions", "yarn-resolutions")] {
        if let Some(obj) = doc.get(field).and_then(|v| v.as_object()) {
            for (name, body) in obj {
                let replacement = match body {
                    serde_json::Value::String(s) => s.clone(),
                    _ => body.to_string(),
                };
                patches.push(Patch {
                    source: source.to_string(),
                    name: name.clone(),
                    replacement,
                });
            }
        }
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members,
        patches,
    })
}

fn read_member(root: &Path, rel: &str) -> Option<Member> {
    let manifest = root.join(rel).join("package.json");
    let raw = std::fs::read_to_string(&manifest).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(member_from_doc(rel, &doc))
}

fn member_from_doc(rel: &str, doc: &serde_json::Value) -> Member {
    let pkg_name = doc
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(rel)
        .to_string();
    let license = doc
        .get("license")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            // Older convention: `"licenses": [{"type": "MIT", ...}]`.
            doc.get("licenses")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|first| first.get("type").and_then(|v| v.as_str()))
                .map(str::to_string)
        });
    let deps = collect_deps_from_doc(doc);
    Member {
        path: rel.to_string(),
        name: pkg_name,
        license,
        deps,
    }
}

fn collect_deps_from_doc(doc: &serde_json::Value) -> Vec<Dep> {
    let mut out: Vec<Dep> = Vec::new();
    for (field, kind) in [
        ("dependencies", DepKind::Normal),
        ("devDependencies", DepKind::Dev),
        ("peerDependencies", DepKind::Peer),
        ("optionalDependencies", DepKind::Optional),
        ("bundledDependencies", DepKind::Other("bundled")),
        ("bundleDependencies", DepKind::Other("bundled")),
    ] {
        let Some(obj) = doc.get(field).and_then(|v| v.as_object()) else {
            continue;
        };
        for (name, value) in obj {
            out.push(parse_dep(name, value, kind));
        }
    }
    out
}

fn parse_dep(name: &str, value: &serde_json::Value, kind: DepKind) -> Dep {
    // `bundledDependencies` is an array of strings, not an object; the loop
    // above only iterates objects, but we still handle the case here for
    // robustness.
    let version_str = value.as_str().map(str::to_string);
    let local_path = version_str.as_ref().and_then(|v| {
        ["file:", "link:", "portal:"]
            .iter()
            .find_map(|p| v.strip_prefix(p).map(str::to_string))
    });
    Dep {
        name: name.to_string(),
        resolved: None,
        version: version_str,
        features: Vec::new(),
        optional: matches!(kind, DepKind::Optional),
        local_path,
        kind,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, content: &str) {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, content).unwrap();
    }

    #[test]
    fn parses_single_package_with_dev_and_peer() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{
              "name": "my-app",
              "version": "1.0.0",
              "dependencies": { "react": "^18.0.0", "lodash": "4.17.21" },
              "devDependencies": { "typescript": "5.4.0" },
              "peerDependencies": { "react-dom": "^18" }
            }"#,
        );
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "node");
        let m = &ws.members[0];
        assert_eq!(m.name, "my-app");
        assert!(m.deps.iter().any(|d| d.name == "react" && d.kind == DepKind::Normal));
        assert!(m.deps.iter().any(|d| d.name == "typescript" && d.kind == DepKind::Dev));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "react-dom" && d.kind == DepKind::Peer));
    }

    #[test]
    fn expands_yarn_workspace_packages_glob() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{
              "name": "monorepo",
              "private": true,
              "workspaces": ["packages/*"]
            }"#,
        );
        write(
            dir.path(),
            "packages/alpha/package.json",
            r#"{ "name": "@org/alpha", "dependencies": { "react": "18" } }"#,
        );
        write(
            dir.path(),
            "packages/beta/package.json",
            r#"{ "name": "@org/beta", "dependencies": { "react": "18", "vue": "3" } }"#,
        );
        let ws = parse(dir.path()).unwrap();
        let names: Vec<&str> = ws.members.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"@org/alpha"));
        assert!(names.contains(&"@org/beta"));
    }

    #[test]
    fn detects_file_link_paths() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{
              "name": "app",
              "dependencies": { "local-lib": "file:../local-lib" }
            }"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        let dep = m.deps.iter().find(|d| d.name == "local-lib").unwrap();
        assert_eq!(dep.local_path.as_deref(), Some("../local-lib"));
    }

    #[test]
    fn captures_npm_overrides() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{
              "name": "app",
              "overrides": { "lodash": "4.17.21" }
            }"#,
        );
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.patches.len(), 1);
        assert_eq!(ws.patches[0].name, "lodash");
        assert_eq!(ws.patches[0].source, "npm-overrides");
    }
}
