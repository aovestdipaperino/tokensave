//! Rust ecosystem parser — `Cargo.toml` + workspace member resolution.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{expand_workspace_globs, Dep, DepKind, Member, Patch, Workspace};

const ECOSYSTEM: &str = "rust";

pub fn detect(root: &Path) -> bool {
    root.join("Cargo.toml").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let root_toml = root.join("Cargo.toml");
    let raw = std::fs::read_to_string(&root_toml).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", root_toml.display()),
    })?;
    let doc: toml::Value = toml::from_str(&raw).map_err(|e| TokenSaveError::Config {
        message: format!("failed to parse {}: {e}", root_toml.display()),
    })?;

    let workspace_table = doc.get("workspace").and_then(|v| v.as_table());

    let mut members: Vec<Member> = Vec::new();

    // The root can be both a workspace and a package.
    if doc.get("package").is_some() {
        if let Some(m) = member_from_doc(".", &doc) {
            members.push(m);
        }
    }

    if let Some(ws) = workspace_table {
        let raw_patterns: Vec<String> = ws
            .get("members")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let excludes: Vec<String> = ws
            .get("exclude")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| format!("!{s}")))
                    .collect()
            })
            .unwrap_or_default();
        let combined: Vec<String> = raw_patterns.into_iter().chain(excludes).collect();
        for member_path in expand_workspace_globs(root, &combined, "Cargo.toml") {
            if let Some(m) = read_member(root, &member_path) {
                members.push(m);
            }
        }
    }

    let patches = collect_patches(&doc);

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members,
        patches,
    })
}

fn read_member(root: &Path, rel: &str) -> Option<Member> {
    let manifest = root.join(rel).join("Cargo.toml");
    let raw = std::fs::read_to_string(&manifest).ok()?;
    let doc: toml::Value = toml::from_str(&raw).ok()?;
    member_from_doc(rel, &doc)
}

fn member_from_doc(rel: &str, doc: &toml::Value) -> Option<Member> {
    let pkg = doc.get("package").and_then(|v| v.as_table())?;
    let pkg_name = pkg.get("name").and_then(|v| v.as_str())?.to_string();
    let license = pkg
        .get("license")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            pkg.get("license-file")
                .and_then(|v| v.as_str())
                .map(|p| format!("file:{p}"))
        });
    let deps = collect_deps_from_doc(doc);
    Some(Member {
        path: rel.to_string(),
        name: pkg_name,
        license,
        deps,
    })
}

fn collect_deps_from_doc(doc: &toml::Value) -> Vec<Dep> {
    let mut out: Vec<Dep> = Vec::new();
    for (section, kind) in [
        ("dependencies", DepKind::Normal),
        ("dev-dependencies", DepKind::Dev),
        ("build-dependencies", DepKind::Build),
    ] {
        if let Some(tbl) = doc.get(section).and_then(|v| v.as_table()) {
            for (name, value) in tbl {
                out.push(parse_dep(name, value, kind));
            }
        }
    }
    if let Some(targets) = doc.get("target").and_then(|v| v.as_table()) {
        for cfg_tbl in targets.values() {
            let Some(cfg) = cfg_tbl.as_table() else {
                continue;
            };
            for (section, kind) in [
                ("dependencies", DepKind::Normal),
                ("dev-dependencies", DepKind::Dev),
                ("build-dependencies", DepKind::Build),
            ] {
                if let Some(tbl) = cfg.get(section).and_then(|v| v.as_table()) {
                    for (name, value) in tbl {
                        out.push(parse_dep(name, value, kind));
                    }
                }
            }
        }
    }
    out
}

fn parse_dep(name: &str, value: &toml::Value, kind: DepKind) -> Dep {
    match value {
        toml::Value::String(v) => Dep {
            name: name.to_string(),
            resolved: None,
            version: Some(v.clone()),
            features: Vec::new(),
            optional: false,
            local_path: None,
            kind,
        },
        toml::Value::Table(t) => {
            let version = t
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let features = t
                .get("features")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let optional = t
                .get("optional")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            let path = t.get("path").and_then(|v| v.as_str()).map(str::to_string);
            Dep {
                name: name.to_string(),
                resolved: None,
                version,
                features,
                optional,
                local_path: path,
                kind,
            }
        }
        _ => Dep {
            name: name.to_string(),
            resolved: None,
            version: None,
            features: Vec::new(),
            optional: false,
            local_path: None,
            kind,
        },
    }
}

fn collect_patches(doc: &toml::Value) -> Vec<Patch> {
    let mut out = Vec::new();
    let Some(patch) = doc.get("patch").and_then(|v| v.as_table()) else {
        return out;
    };
    for (source, tbl) in patch {
        let Some(crates) = tbl.as_table() else {
            continue;
        };
        for (crate_name, body) in crates {
            let replacement = match body {
                toml::Value::String(s) => s.clone(),
                toml::Value::Table(t) => {
                    let mut bits = Vec::new();
                    if let Some(p) = t.get("path").and_then(|v| v.as_str()) {
                        bits.push(format!("path = \"{p}\""));
                    }
                    if let Some(g) = t.get("git").and_then(|v| v.as_str()) {
                        bits.push(format!("git = \"{g}\""));
                    }
                    if let Some(v) = t.get("version").and_then(|v| v.as_str()) {
                        bits.push(format!("version = \"{v}\""));
                    }
                    bits.join(", ")
                }
                _ => String::new(),
            };
            out.push(Patch {
                source: source.clone(),
                name: crate_name.clone(),
                replacement,
            });
        }
    }
    out
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
    fn parses_single_crate_dependencies() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            r#"
[package]
name = "solo"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.47", features = ["full", "macros"] }

[dev-dependencies]
tempfile = "3"
"#,
        );

        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "rust");
        assert_eq!(ws.members.len(), 1);
        let m = &ws.members[0];
        assert_eq!(m.name, "solo");
        let serde = m.deps.iter().find(|d| d.name == "serde").unwrap();
        assert_eq!(serde.kind, DepKind::Normal);
        assert_eq!(serde.version.as_deref(), Some("1.0"));
        let tokio = m.deps.iter().find(|d| d.name == "tokio").unwrap();
        assert_eq!(tokio.features, vec!["full", "macros"]);
        let tempfile = m.deps.iter().find(|d| d.name == "tempfile").unwrap();
        assert_eq!(tempfile.kind, DepKind::Dev);
    }

    #[test]
    fn expands_workspace_member_glob() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        write(
            dir.path(),
            "crates/alpha/Cargo.toml",
            r#"
[package]
name = "alpha"
version = "0.1.0"
"#,
        );
        write(
            dir.path(),
            "crates/beta/Cargo.toml",
            r#"
[package]
name = "beta"
version = "0.1.0"
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let names: Vec<&str> = ws.members.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn collects_target_specific_deps() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            r#"
[package]
name = "x"
version = "0.1.0"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert!(m.deps.iter().any(|d| d.name == "libc"));
    }

    #[test]
    fn parses_patches() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            r#"
[workspace]
members = []

[patch.crates-io]
bytes = { path = "vendor/bytes" }
"#,
        );
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.patches.len(), 1);
        assert_eq!(ws.patches[0].source, "crates-io");
        assert_eq!(ws.patches[0].name, "bytes");
        assert!(ws.patches[0].replacement.contains("vendor/bytes"));
    }
}
