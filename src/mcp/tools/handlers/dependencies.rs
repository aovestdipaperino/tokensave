//! `tokensave_dependencies` — Rust package-manifest introspection (first
//! iteration of issue #105).
//!
//! Scope (per issue #105): parse `Cargo.toml` at the project root, resolve
//! workspace members, expose declared dependencies (`[dependencies]`,
//! `[dev-dependencies]`, `[build-dependencies]`). Lockfile resolution and
//! non-Rust ecosystems (`package.json`, `pyproject.toml`, `go.mod`, …) are
//! intentionally deferred to follow-up issues to keep this PR small.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use super::super::ToolResult;
use super::truncate_response;

const KIND_NORMAL: &str = "normal";
const KIND_DEV: &str = "dev";
const KIND_BUILD: &str = "build";

/// One declared dependency, normalized across simple-string (`crate = "1.0"`)
/// and table (`crate = { version = "1.0", features = [...] }`) forms.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Dep {
    crate_name: String,
    version: Option<String>,
    features: Vec<String>,
    optional: bool,
    /// Set when the dep is declared via `path = "..."` (workspace-local).
    path: Option<String>,
    kind: &'static str,
}

/// Handles `tokensave_dependencies` tool calls.
///
/// Modes (zero-input is the default workspace summary):
/// - no input → workspace summary: members + every declared dep in any member
/// - `crate: X` → list members that depend on `X` (with kind/features/version)
/// - `member: X` → list every dep declared by member `X`
///
/// All filesystem reads are blocking — kept `async` for routing parity with
/// the other MCP handlers.
#[allow(clippy::unused_async)]
pub(super) async fn handle_dependencies(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let crate_name = args.get("crate").and_then(|v| v.as_str());
    let member_name = args.get("member").and_then(|v| v.as_str());
    let kind_filter = args
        .get("kind")
        .and_then(|v| v.as_str())
        .filter(|s| matches!(*s, "normal" | "dev" | "build" | "all"))
        .unwrap_or("all");

    let workspace = parse_workspace(cg.project_root())?;

    if let Some(name) = member_name {
        return Ok(render_member(&workspace, name, kind_filter));
    }
    if let Some(name) = crate_name {
        return Ok(render_crate(&workspace, name, kind_filter));
    }
    Ok(render_summary(&workspace, kind_filter))
}

#[derive(Debug)]
struct Workspace {
    root: PathBuf,
    members: Vec<Member>,
    /// Patches at the workspace root (`[patch.crates-io]`, etc.). Stored as
    /// `(source, crate_name, replacement)` so callers can spot overrides.
    patches: Vec<(String, String, String)>,
}

#[derive(Debug)]
struct Member {
    /// Path relative to the workspace root (`.` for a single-crate project).
    path: String,
    package_name: String,
    deps: Vec<Dep>,
}

fn parse_workspace(root: &Path) -> Result<Workspace> {
    let root_toml = root.join("Cargo.toml");
    if !root_toml.exists() {
        return Err(TokenSaveError::Config {
            message: format!(
                "Cargo.toml not found at {} — tokensave_dependencies currently \
                 supports Rust projects only (issue #105 will expand to other \
                 ecosystems in follow-up iterations)",
                root.display()
            ),
        });
    }

    let raw = std::fs::read_to_string(&root_toml).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", root_toml.display()),
    })?;
    let doc: toml::Value = toml::from_str(&raw).map_err(|e| TokenSaveError::Config {
        message: format!("failed to parse {}: {e}", root_toml.display()),
    })?;

    let workspace_table = doc.get("workspace").and_then(|v| v.as_table());

    let mut members: Vec<Member> = Vec::new();

    // Treat the root as a member when it has its own [package].
    if doc.get("package").is_some() {
        if let Some(m) = member_from_doc(".", &doc) {
            members.push(m);
        }
    }

    if let Some(ws) = workspace_table {
        if let Some(arr) = ws.get("members").and_then(|v| v.as_array()) {
            for entry in arr {
                if let Some(pattern) = entry.as_str() {
                    for member_path in expand_member_pattern(root, pattern) {
                        if let Some(m) = read_member(root, &member_path) {
                            members.push(m);
                        }
                    }
                }
            }
        }
    }

    let patches = collect_patches(&doc);

    Ok(Workspace {
        root: root.to_path_buf(),
        members,
        patches,
    })
}

/// Expand `members = ["crates/*"]`-style globs. Only handles the trailing
/// `*` pattern (the common case) — anything more exotic is passed through
/// literally and resolved as a directory path.
fn expand_member_pattern(root: &Path, pattern: &str) -> Vec<String> {
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let dir = root.join(prefix);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out: Vec<String> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let rel = format!(
                    "{}/{}",
                    prefix,
                    e.file_name().to_string_lossy().into_owned()
                );
                e.path().join("Cargo.toml").exists().then_some(rel)
            })
            .collect();
        out.sort();
        return out;
    }
    if pattern.contains('*') {
        // Unsupported glob shape — skip rather than guess.
        return Vec::new();
    }
    vec![pattern.to_string()]
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
    let deps = collect_deps_from_doc(doc);
    Some(Member {
        path: rel.to_string(),
        package_name: pkg_name,
        deps,
    })
}

fn collect_deps_from_doc(doc: &toml::Value) -> Vec<Dep> {
    let mut out: Vec<Dep> = Vec::new();
    for (section, kind) in [
        ("dependencies", KIND_NORMAL),
        ("dev-dependencies", KIND_DEV),
        ("build-dependencies", KIND_BUILD),
    ] {
        if let Some(tbl) = doc.get(section).and_then(|v| v.as_table()) {
            for (name, value) in tbl {
                out.push(parse_dep(name, value, kind));
            }
        }
    }
    // Target-specific dependencies under `[target.<cfg>.dependencies]`.
    if let Some(targets) = doc.get("target").and_then(|v| v.as_table()) {
        for cfg_tbl in targets.values() {
            let Some(cfg) = cfg_tbl.as_table() else {
                continue;
            };
            for (section, kind) in [
                ("dependencies", KIND_NORMAL),
                ("dev-dependencies", KIND_DEV),
                ("build-dependencies", KIND_BUILD),
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

fn parse_dep(name: &str, value: &toml::Value, kind: &'static str) -> Dep {
    match value {
        toml::Value::String(v) => Dep {
            crate_name: name.to_string(),
            version: Some(v.clone()),
            features: Vec::new(),
            optional: false,
            path: None,
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
            let path = t
                .get("path")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Dep {
                crate_name: name.to_string(),
                version,
                features,
                optional,
                path,
                kind,
            }
        }
        _ => Dep {
            crate_name: name.to_string(),
            version: None,
            features: Vec::new(),
            optional: false,
            path: None,
            kind,
        },
    }
}

fn collect_patches(doc: &toml::Value) -> Vec<(String, String, String)> {
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
            out.push((source.clone(), crate_name.clone(), replacement));
        }
    }
    out
}

fn dep_to_json(d: &Dep) -> Value {
    json!({
        "crate": d.crate_name,
        "kind": d.kind,
        "version": d.version,
        "features": d.features,
        "optional": d.optional,
        "path": d.path,
    })
}

fn kind_passes(dep: &Dep, filter: &str) -> bool {
    matches!(filter, "all") || dep.kind == filter
}

fn render_summary(ws: &Workspace, kind_filter: &str) -> ToolResult {
    let mut all_crates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut total_members = 0u64;
    for m in &ws.members {
        total_members += 1;
        for d in &m.deps {
            if kind_passes(d, kind_filter) {
                all_crates
                    .entry(d.crate_name.clone())
                    .or_default()
                    .insert(m.package_name.clone());
            }
        }
    }
    let crate_rows: Vec<Value> = all_crates
        .into_iter()
        .map(|(name, members)| {
            json!({
                "crate": name,
                "used_in": members.into_iter().collect::<Vec<_>>(),
            })
        })
        .collect();
    let member_names: Vec<&str> = ws.members.iter().map(|m| m.package_name.as_str()).collect();
    let patch_rows: Vec<Value> = ws
        .patches
        .iter()
        .map(|(src, name, replacement)| {
            json!({ "source": src, "crate": name, "replacement": replacement })
        })
        .collect();
    let output = json!({
        "mode": "workspace",
        "root": ws.root.display().to_string(),
        "member_count": total_members,
        "members": member_names,
        "kind_filter": kind_filter,
        "crates": crate_rows,
        "patches": patch_rows,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![],
    }
}

fn render_member(ws: &Workspace, name: &str, kind_filter: &str) -> ToolResult {
    let member = ws
        .members
        .iter()
        .find(|m| m.package_name == name || m.path == name);
    let Some(m) = member else {
        let known: Vec<&str> = ws.members.iter().map(|m| m.package_name.as_str()).collect();
        let formatted = serde_json::to_string_pretty(&json!({
            "mode": "member",
            "error": format!("no member named '{name}'"),
            "available_members": known,
        }))
        .unwrap_or_default();
        return ToolResult {
            value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
            touched_files: vec![],
        };
    };

    let deps: Vec<Value> = m
        .deps
        .iter()
        .filter(|d| kind_passes(d, kind_filter))
        .map(dep_to_json)
        .collect();
    let output = json!({
        "mode": "member",
        "member": m.package_name,
        "path": m.path,
        "kind_filter": kind_filter,
        "dependency_count": deps.len(),
        "dependencies": deps,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![format!("{}/Cargo.toml", m.path)],
    }
}

fn render_crate(ws: &Workspace, name: &str, kind_filter: &str) -> ToolResult {
    let mut rows: Vec<Value> = Vec::new();
    for m in &ws.members {
        for d in &m.deps {
            if d.crate_name != name {
                continue;
            }
            if !kind_passes(d, kind_filter) {
                continue;
            }
            rows.push(json!({
                "member": m.package_name,
                "path": m.path,
                "kind": d.kind,
                "version": d.version,
                "features": d.features,
                "optional": d.optional,
                "local_path": d.path,
            }));
        }
    }
    let output = json!({
        "mode": "crate",
        "crate": name,
        "kind_filter": kind_filter,
        "usage_count": rows.len(),
        "usages": rows,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![],
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

        let ws = parse_workspace(dir.path()).unwrap();
        assert_eq!(ws.members.len(), 1);
        let m = &ws.members[0];
        assert_eq!(m.package_name, "solo");
        let serde = m.deps.iter().find(|d| d.crate_name == "serde").unwrap();
        assert_eq!(serde.kind, KIND_NORMAL);
        assert_eq!(serde.version.as_deref(), Some("1.0"));
        let tokio = m.deps.iter().find(|d| d.crate_name == "tokio").unwrap();
        assert_eq!(tokio.features, vec!["full", "macros"]);
        let tempfile = m.deps.iter().find(|d| d.crate_name == "tempfile").unwrap();
        assert_eq!(tempfile.kind, KIND_DEV);
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

[dependencies]
serde = "1"
"#,
        );
        write(
            dir.path(),
            "crates/beta/Cargo.toml",
            r#"
[package]
name = "beta"
version = "0.1.0"

[dependencies]
serde = "1"
tokio = "1"
"#,
        );

        let ws = parse_workspace(dir.path()).unwrap();
        let names: Vec<&str> = ws.members.iter().map(|m| m.package_name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn parses_workspace_patches() {
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

        let ws = parse_workspace(dir.path()).unwrap();
        assert_eq!(ws.patches.len(), 1);
        let (src, name, replacement) = &ws.patches[0];
        assert_eq!(src, "crates-io");
        assert_eq!(name, "bytes");
        assert!(replacement.contains("vendor/bytes"));
    }

    #[test]
    fn parse_dep_handles_table_with_path() {
        let raw: toml::Value = toml::from_str(
            r#"
local_lib = { path = "../local", version = "0.1" }
"#,
        )
        .unwrap();
        let value = raw.get("local_lib").unwrap();
        let dep = parse_dep("local_lib", value, KIND_NORMAL);
        assert_eq!(dep.path.as_deref(), Some("../local"));
        assert_eq!(dep.version.as_deref(), Some("0.1"));
    }

    #[test]
    fn collects_target_specific_deps() {
        let doc: toml::Value = toml::from_str(
            r#"
[package]
name = "x"
version = "0.1.0"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#,
        )
        .unwrap();
        let deps = collect_deps_from_doc(&doc);
        assert!(
            deps.iter().any(|d| d.crate_name == "libc"),
            "target-cfg deps should be included"
        );
    }

    #[test]
    fn errors_when_cargo_toml_missing() {
        let dir = TempDir::new().unwrap();
        let err = parse_workspace(dir.path()).unwrap_err();
        assert!(err.to_string().contains("Cargo.toml not found"));
    }
}
