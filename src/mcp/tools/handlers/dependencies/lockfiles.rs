//! Lockfile parsers for every ecosystem with a stable lockfile format.
//!
//! Each `apply_*` function reads the relevant lockfile and writes the
//! resolved version back into each member's deps (matching by name). When
//! the lockfile is absent or unparseable, the deps are left unchanged — the
//! handler still returns useful data.
//!
//! YAML-format lockfiles (`pnpm-lock.yaml`, `pubspec.lock`) are handled by
//! the YAML phase elsewhere.

use std::collections::HashMap;

use super::common::Workspace;

/// Dispatch to the right `apply_*` based on the workspace's ecosystem.
pub fn apply_to_workspace(ws: &mut Workspace) {
    match ws.ecosystem {
        "rust" => apply_cargo_lock(ws),
        "node" => apply_node_lock(ws),
        "python" => apply_python_lock(ws),
        "go" => apply_go_sum(ws),
        "dotnet" => apply_dotnet_lock(ws),
        "php" => apply_composer_lock(ws),
        "ruby" => apply_gemfile_lock(ws),
        "dart" => super::dart::apply_lockfile(ws),
        "crystal" => super::crystal::apply_lockfile(ws),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Rust — Cargo.lock (TOML)
// ---------------------------------------------------------------------------

fn apply_cargo_lock(ws: &mut Workspace) {
    let path = ws.root.join("Cargo.lock");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(doc) = toml::from_str::<toml::Value>(&raw) else {
        return;
    };
    // `Cargo.lock` is `[[package]]` array with `name`/`version` entries.
    let mut versions: HashMap<String, String> = HashMap::new();
    if let Some(arr) = doc.get("package").and_then(|v| v.as_array()) {
        for pkg in arr {
            let (Some(name), Some(version)) = (
                pkg.get("name").and_then(|v| v.as_str()),
                pkg.get("version").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            versions.insert(name.to_string(), version.to_string());
        }
    }
    fill_resolved(ws, &versions);
}

// ---------------------------------------------------------------------------
// Node — package-lock.json / yarn.lock
// ---------------------------------------------------------------------------

fn apply_node_lock(ws: &mut Workspace) {
    if let Ok(raw) = std::fs::read_to_string(ws.root.join("package-lock.json")) {
        if let Ok(versions) = parse_package_lock(&raw) {
            fill_resolved(ws, &versions);
            return;
        }
    }
    if let Ok(raw) = std::fs::read_to_string(ws.root.join("yarn.lock")) {
        let versions = parse_yarn_lock(&raw);
        if !versions.is_empty() {
            fill_resolved(ws, &versions);
            return;
        }
    }
    if let Ok(raw) = std::fs::read_to_string(ws.root.join("pnpm-lock.yaml")) {
        let versions = parse_pnpm_lock(&raw);
        if !versions.is_empty() {
            fill_resolved(ws, &versions);
        }
    }
}

/// `pnpm-lock.yaml` schema: top-level `packages:` map keyed by
/// `/<name>@<version>` (lockfile v6) or `/<name>/<version>` (v5/earlier).
fn parse_pnpm_lock(raw: &str) -> HashMap<String, String> {
    use super::yaml_util::{node_field, parse_root};
    let Some(root) = parse_root(raw) else {
        return HashMap::new();
    };
    let Some(packages) = node_field(&root, "packages") else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (key, _info) in super::yaml_util::hash_entries(packages) {
        // key examples:
        //   "/react@18.3.1"  →  ("react", "18.3.1")
        //   "/@scope/pkg@1.2.3"
        //   "/react/18.3.1"  (older format)
        let trimmed = key.trim_start_matches('/');
        let (name, version) = if let Some(idx) = trimmed.rfind('@').filter(|&i| i > 0) {
            (&trimmed[..idx], &trimmed[idx + 1..])
        } else if let Some(idx) = trimmed.rfind('/') {
            (&trimmed[..idx], &trimmed[idx + 1..])
        } else {
            continue;
        };
        // Strip peer-dep suffix `(react@18)` from version.
        let version = version.split('(').next().unwrap_or(version);
        if name.is_empty() || version.is_empty() {
            continue;
        }
        out.entry(name.to_string())
            .or_insert_with(|| version.to_string());
    }
    out
}

fn parse_package_lock(raw: &str) -> Result<HashMap<String, String>, serde_json::Error> {
    let doc: serde_json::Value = serde_json::from_str(raw)?;
    let mut out: HashMap<String, String> = HashMap::new();
    // lockfile v2+: `packages` map keyed by relative path; root is "".
    if let Some(pkgs) = doc.get("packages").and_then(|v| v.as_object()) {
        for (key, info) in pkgs {
            if key.is_empty() {
                continue;
            }
            let name = if let Some(stripped) = key.rsplit_once("node_modules/") {
                stripped.1.to_string()
            } else {
                key.clone()
            };
            if let Some(v) = info.get("version").and_then(|v| v.as_str()) {
                out.entry(name).or_insert_with(|| v.to_string());
            }
        }
    }
    // lockfile v1: `dependencies` recursive object.
    if out.is_empty() {
        if let Some(deps) = doc.get("dependencies").and_then(|v| v.as_object()) {
            for (name, info) in deps {
                if let Some(v) = info.get("version").and_then(|v| v.as_str()) {
                    out.entry(name.clone()).or_insert_with(|| v.to_string());
                }
            }
        }
    }
    Ok(out)
}

/// Minimal yarn.lock parser: entries are blocks separated by a blank line.
/// Block header is one or more comma-separated dep specs like
/// `"foo@^1.0", "foo@~1.0":`. We grab the package name from the first spec
/// and the `  version "1.2.3"` line.
fn parse_yarn_lock(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut current_name: Option<String> = None;
    for line in raw.lines() {
        if line.is_empty() || line.starts_with('#') {
            current_name = None;
            continue;
        }
        if !line.starts_with(' ') && line.trim_end().ends_with(':') {
            // Block header.
            let header = line.trim_end().trim_end_matches(':');
            // First spec is `"name@range"` or `name@range`.
            let first = header.split(',').next().unwrap_or(header).trim();
            let first = first.trim_matches('"');
            // `@scope/pkg@range` — name is everything before the LAST '@'.
            let at_pos = first.rfind('@').filter(|&i| i > 0);
            let name = match at_pos {
                Some(i) => first[..i].to_string(),
                None => first.to_string(),
            };
            current_name = Some(name);
            continue;
        }
        if let Some(rest) = line.trim_start().strip_prefix("version ") {
            let v = rest.trim().trim_matches('"');
            if let Some(name) = current_name.as_ref() {
                out.entry(name.clone()).or_insert_with(|| v.to_string());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Python — poetry.lock / uv.lock (TOML) / Pipfile.lock (JSON)
// ---------------------------------------------------------------------------

fn apply_python_lock(ws: &mut Workspace) {
    for candidate in ["poetry.lock", "uv.lock"] {
        if let Ok(raw) = std::fs::read_to_string(ws.root.join(candidate)) {
            if let Ok(doc) = toml::from_str::<toml::Value>(&raw) {
                let mut versions = HashMap::new();
                if let Some(arr) = doc.get("package").and_then(|v| v.as_array()) {
                    for pkg in arr {
                        if let (Some(n), Some(v)) = (
                            pkg.get("name").and_then(|v| v.as_str()),
                            pkg.get("version").and_then(|v| v.as_str()),
                        ) {
                            versions.insert(n.to_string(), v.to_string());
                        }
                    }
                }
                if !versions.is_empty() {
                    fill_resolved(ws, &versions);
                    return;
                }
            }
        }
    }
    if let Ok(raw) = std::fs::read_to_string(ws.root.join("Pipfile.lock")) {
        if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&raw) {
            let mut versions = HashMap::new();
            for section in ["default", "develop"] {
                if let Some(obj) = doc.get(section).and_then(|v| v.as_object()) {
                    for (name, info) in obj {
                        // `version` is typically "==1.2.3" — strip the leading `==`.
                        if let Some(v) = info.get("version").and_then(|v| v.as_str()) {
                            let clean = v.trim_start_matches("==").to_string();
                            versions.insert(name.clone(), clean);
                        }
                    }
                }
            }
            fill_resolved(ws, &versions);
        }
    }
}

// ---------------------------------------------------------------------------
// Go — go.sum (each line: "name version[/go.mod] hash")
// ---------------------------------------------------------------------------

fn apply_go_sum(ws: &mut Workspace) {
    let Ok(raw) = std::fs::read_to_string(ws.root.join("go.sum")) else {
        return;
    };
    let mut versions: HashMap<String, String> = HashMap::new();
    for line in raw.lines() {
        let mut tokens = line.split_whitespace();
        let Some(name) = tokens.next() else {
            continue;
        };
        let Some(version) = tokens.next() else {
            continue;
        };
        // Skip the `name v1.0.0/go.mod` lines — keep the bare version one.
        if version.ends_with("/go.mod") {
            continue;
        }
        versions.insert(name.to_string(), version.to_string());
    }
    fill_resolved(ws, &versions);
}

// ---------------------------------------------------------------------------
// .NET — packages.lock.json (JSON)
// ---------------------------------------------------------------------------

fn apply_dotnet_lock(ws: &mut Workspace) {
    let Ok(raw) = std::fs::read_to_string(ws.root.join("packages.lock.json")) else {
        return;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    let mut versions: HashMap<String, String> = HashMap::new();
    // Schema: `dependencies.<target-framework>.<package> = { resolved: "..." }`
    if let Some(targets) = doc.get("dependencies").and_then(|v| v.as_object()) {
        for tfm in targets.values() {
            let Some(obj) = tfm.as_object() else {
                continue;
            };
            for (name, info) in obj {
                if let Some(v) = info.get("resolved").and_then(|v| v.as_str()) {
                    versions
                        .entry(name.clone())
                        .or_insert_with(|| v.to_string());
                }
            }
        }
    }
    fill_resolved(ws, &versions);
}

// ---------------------------------------------------------------------------
// PHP — composer.lock (JSON)
// ---------------------------------------------------------------------------

fn apply_composer_lock(ws: &mut Workspace) {
    let Ok(raw) = std::fs::read_to_string(ws.root.join("composer.lock")) else {
        return;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    let mut versions: HashMap<String, String> = HashMap::new();
    for section in ["packages", "packages-dev"] {
        let Some(arr) = doc.get(section).and_then(|v| v.as_array()) else {
            continue;
        };
        for pkg in arr {
            if let (Some(n), Some(v)) = (
                pkg.get("name").and_then(|v| v.as_str()),
                pkg.get("version").and_then(|v| v.as_str()),
            ) {
                versions.insert(n.to_string(), v.to_string());
            }
        }
    }
    fill_resolved(ws, &versions);
}

// ---------------------------------------------------------------------------
// Ruby — Gemfile.lock (custom line-based)
// ---------------------------------------------------------------------------

fn apply_gemfile_lock(ws: &mut Workspace) {
    let Ok(raw) = std::fs::read_to_string(ws.root.join("Gemfile.lock")) else {
        return;
    };
    let mut versions: HashMap<String, String> = HashMap::new();
    let mut in_specs = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "specs:" {
            in_specs = true;
            continue;
        }
        if line.starts_with(char::is_alphabetic) {
            // New top-level section (`PLATFORMS`, `DEPENDENCIES`, ...).
            in_specs = false;
            continue;
        }
        if !in_specs {
            continue;
        }
        // Spec line looks like:
        //   `    rails (7.1.0)`     ← top-level gem
        //   `      actionpack (= 7.1.0)`  ← dependency of the gem above
        if !line.starts_with("    ") || line.starts_with("      ") {
            continue;
        }
        let body = trimmed;
        let Some(open) = body.find('(') else {
            continue;
        };
        let Some(close) = body.rfind(')') else {
            continue;
        };
        let name = body[..open].trim().to_string();
        let version = body[open + 1..close].trim().to_string();
        if !name.is_empty() && !version.is_empty() {
            versions.insert(name, version);
        }
    }
    fill_resolved(ws, &versions);
}

// ---------------------------------------------------------------------------
// Helper: stamp resolved versions onto every member's deps.
// ---------------------------------------------------------------------------

fn fill_resolved(ws: &mut Workspace, versions: &HashMap<String, String>) {
    for m in &mut ws.members {
        for d in &mut m.deps {
            if d.resolved.is_some() {
                continue;
            }
            if let Some(v) = versions.get(&d.name) {
                d.resolved = Some(v.clone());
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::mcp::tools::handlers::dependencies::common::{Dep, DepKind, Member, Workspace};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn fixture(ecosystem: &'static str, root: &Path, dep_names: &[&str]) -> Workspace {
        Workspace {
            ecosystem,
            root: root.to_path_buf(),
            members: vec![Member {
                path: ".".to_string(),
                name: "test".to_string(),
                license: None,
                deps: dep_names
                    .iter()
                    .map(|n| Dep {
                        name: (*n).to_string(),
                        resolved: None,
                        version: None,
                        features: vec![],
                        optional: false,
                        local_path: None,
                        kind: DepKind::Normal,
                    })
                    .collect(),
            }],
            patches: vec![],
        }
    }

    #[test]
    fn cargo_lock_stamps_resolved_versions() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.lock"),
            r#"
[[package]]
name = "serde"
version = "1.0.219"

[[package]]
name = "tokio"
version = "1.47.1"
"#,
        )
        .unwrap();
        let mut ws = fixture("rust", dir.path(), &["serde", "tokio", "missing"]);
        apply_to_workspace(&mut ws);
        let resolved: Vec<_> = ws.members[0]
            .deps
            .iter()
            .map(|d| (d.name.as_str(), d.resolved.clone()))
            .collect();
        assert_eq!(resolved[0], ("serde", Some("1.0.219".to_string())));
        assert_eq!(resolved[1], ("tokio", Some("1.47.1".to_string())));
        assert_eq!(resolved[2], ("missing", None));
    }

    #[test]
    fn package_lock_v2_stamps_versions() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package-lock.json"),
            r#"{
              "name": "x", "version": "0.0.0", "lockfileVersion": 3,
              "packages": {
                "": { "name": "x", "version": "0.0.0" },
                "node_modules/react": { "version": "18.3.1" }
              }
            }"#,
        )
        .unwrap();
        let mut ws = fixture("node", dir.path(), &["react"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("18.3.1"));
    }

    #[test]
    fn yarn_lock_stamps_versions() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("yarn.lock"),
            r#"
"react@^18.0.0":
  version "18.3.1"
  resolved "https://registry.yarnpkg.com/react/-/react-18.3.1.tgz"

"@scope/pkg@^1.0.0":
  version "1.2.3"
"#,
        )
        .unwrap();
        let mut ws = fixture("node", dir.path(), &["react", "@scope/pkg"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("18.3.1"));
        assert_eq!(ws.members[0].deps[1].resolved.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn poetry_lock_stamps_versions() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("poetry.lock"),
            r#"
[[package]]
name = "requests"
version = "2.31.0"
"#,
        )
        .unwrap();
        let mut ws = fixture("python", dir.path(), &["requests"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("2.31.0"));
    }

    #[test]
    fn go_sum_stamps_versions_ignoring_gomod_lines() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("go.sum"),
            r#"
github.com/spf13/cobra v1.8.0 h1:foo
github.com/spf13/cobra v1.8.0/go.mod h1:bar
"#,
        )
        .unwrap();
        let mut ws = fixture("go", dir.path(), &["github.com/spf13/cobra"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("v1.8.0"));
    }

    #[test]
    fn composer_lock_stamps_versions() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("composer.lock"),
            r#"{
              "packages": [
                { "name": "symfony/console", "version": "v7.1.0" }
              ],
              "packages-dev": [
                { "name": "phpunit/phpunit", "version": "10.5.0" }
              ]
            }"#,
        )
        .unwrap();
        let mut ws = fixture("php", dir.path(), &["symfony/console", "phpunit/phpunit"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("v7.1.0"));
        assert_eq!(ws.members[0].deps[1].resolved.as_deref(), Some("10.5.0"));
    }

    #[test]
    fn gemfile_lock_stamps_top_level_gems() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Gemfile.lock"),
            "GEM
  remote: https://rubygems.org/
  specs:
    rails (7.1.0)
      actionpack (= 7.1.0)
    pg (1.5.4)
PLATFORMS
  ruby
DEPENDENCIES
  rails
  pg
",
        )
        .unwrap();
        let mut ws = fixture("ruby", dir.path(), &["rails", "pg"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("7.1.0"));
        assert_eq!(ws.members[0].deps[1].resolved.as_deref(), Some("1.5.4"));
    }

    #[test]
    fn dotnet_packages_lock_stamps_resolved() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("packages.lock.json"),
            r#"{
              "version": 1,
              "dependencies": {
                "net8.0": {
                  "Newtonsoft.Json": { "type": "Direct", "resolved": "13.0.3" }
                }
              }
            }"#,
        )
        .unwrap();
        let mut ws = fixture("dotnet", dir.path(), &["Newtonsoft.Json"]);
        apply_to_workspace(&mut ws);
        assert_eq!(ws.members[0].deps[0].resolved.as_deref(), Some("13.0.3"));
    }

    #[test]
    fn missing_lockfile_is_a_noop() {
        let dir = TempDir::new().unwrap();
        let mut ws = fixture("rust", dir.path(), &["serde"]);
        apply_to_workspace(&mut ws);
        assert!(ws.members[0].deps[0].resolved.is_none());
    }
}
