//! Shared types used by every ecosystem parser.
//!
//! Each ecosystem (Rust / Node / Python / Go / Java / .NET / PHP / Ruby)
//! produces a `Workspace` describing its members and their declared
//! dependencies. The top-level handler renders one shape across ecosystems so
//! the MCP response stays uniform regardless of language.

use std::path::PathBuf;

use serde_json::{json, Value};

/// Conceptual kind of a declared dependency. Each ecosystem maps its own
/// section names to one of these — the labels are not free-form strings to
/// keep `kind` filtering predictable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
    Peer,
    Optional,
    /// Free-form fallback for ecosystem-specific kinds (e.g. Composer
    /// `replace`, Cargo `target-cfg`-scoped reuse).
    Other(&'static str),
}

impl DepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DepKind::Normal => "normal",
            DepKind::Dev => "dev",
            DepKind::Build => "build",
            DepKind::Peer => "peer",
            DepKind::Optional => "optional",
            DepKind::Other(s) => s,
        }
    }

    /// Does this kind match the `--kind` filter passed by the user?
    /// `"all"` matches everything; specific filters match only the named
    /// kind.
    pub fn passes(self, filter: &str) -> bool {
        if filter == "all" {
            return true;
        }
        self.as_str() == filter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    /// Declared version expression (the range/spec from the manifest).
    pub version: Option<String>,
    /// Concrete version resolved from a lockfile, when the user asked for it
    /// (`include_lockfile: true`). Independent of `version`, which keeps the
    /// declared range.
    pub resolved: Option<String>,
    pub features: Vec<String>,
    pub optional: bool,
    /// `path = ".."` / `"file:../local"` / etc. — workspace-local deps.
    pub local_path: Option<String>,
    pub kind: DepKind,
}

#[derive(Debug, Clone)]
pub struct Member {
    /// Path relative to the workspace root. Use `"."` for a single-package
    /// project.
    pub path: String,
    /// Display name of this member/package.
    pub name: String,
    /// Declared license of this member, when the manifest carries one
    /// (Cargo.toml `license`, package.json `license`, composer.json
    /// `license`, pyproject.toml `[project] license`).
    pub license: Option<String>,
    pub deps: Vec<Dep>,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub ecosystem: &'static str,
    pub root: PathBuf,
    pub members: Vec<Member>,
    /// Free-form ecosystem-specific notes (e.g. Cargo `[patch.crates-io]`
    /// entries, Go `replace` directives, npm `overrides`). Rendered verbatim
    /// in the MCP response.
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone)]
pub struct Patch {
    pub source: String,
    pub name: String,
    pub replacement: String,
}

pub fn dep_to_json(d: &Dep) -> Value {
    json!({
        "name": d.name,
        "kind": d.kind.as_str(),
        "version": d.version,
        "resolved": d.resolved,
        "features": d.features,
        "optional": d.optional,
        "path": d.local_path,
    })
}

pub fn patch_to_json(p: &Patch) -> Value {
    json!({
        "source": p.source,
        "name": p.name,
        "replacement": p.replacement,
    })
}

/// Expand a set of workspace-member glob patterns into concrete directory
/// paths (relative to `root`). Each directory is only included if it
/// contains the named `manifest` file.
///
/// Supports:
/// - Literal paths (`crates/foo`) — passed through if the manifest exists.
/// - Trailing-`*` (`crates/*`, `packages/*`) — enumerate children with a
///   manifest.
/// - Intermediate-`*` (`crates/*/bench`) — `*` matches one directory level.
/// - Double-star (`packages/**`) — recursive descent up to a small depth
///   bound to keep filesystem cost reasonable.
/// - Negation (`!packages/excluded`) — removes a previously-matched path.
///   Following the npm/yarn convention, negations apply to the accumulated
///   set in order.
pub fn expand_workspace_globs(
    root: &std::path::Path,
    patterns: &[String],
    manifest: &str,
) -> Vec<String> {
    let mut acc: Vec<String> = Vec::new();
    for raw_pattern in patterns {
        if let Some(stripped) = raw_pattern.strip_prefix('!') {
            let excluded: std::collections::HashSet<String> =
                expand_one_pattern(root, stripped, manifest)
                    .into_iter()
                    .collect();
            acc.retain(|p| !excluded.contains(p));
        } else {
            for p in expand_one_pattern(root, raw_pattern, manifest) {
                if !acc.contains(&p) {
                    acc.push(p);
                }
            }
        }
    }
    acc.sort();
    acc
}

fn expand_one_pattern(root: &std::path::Path, pattern: &str, manifest: &str) -> Vec<String> {
    if !pattern.contains('*') {
        // Literal — accept only when the manifest exists.
        let candidate = root.join(pattern).join(manifest);
        return if candidate.exists() {
            vec![pattern.to_string()]
        } else {
            Vec::new()
        };
    }
    let parts: Vec<&str> = pattern.split('/').collect();
    let mut out: Vec<String> = Vec::new();
    walk_pattern(root, &parts, 0, String::new(), manifest, &mut out);
    out
}

fn walk_pattern(
    root: &std::path::Path,
    parts: &[&str],
    idx: usize,
    accumulated: String,
    manifest: &str,
    out: &mut Vec<String>,
) {
    if idx == parts.len() {
        let candidate = root.join(&accumulated).join(manifest);
        if candidate.exists() {
            out.push(accumulated);
        }
        return;
    }
    let segment = parts[idx];
    let current = root.join(&accumulated);
    if segment == "**" {
        // Match the rest of the pattern at every depth from here downward,
        // capped at 4 levels to keep filesystem traversal bounded.
        recurse_double_star(root, &accumulated, &parts[idx + 1..], manifest, out, 0);
        return;
    }
    if segment == "*" {
        let Ok(entries) = std::fs::read_dir(&current) else {
            return;
        };
        for e in entries.filter_map(std::result::Result::ok) {
            if !e.path().is_dir() {
                continue;
            }
            let child = e.file_name().to_string_lossy().into_owned();
            let next = if accumulated.is_empty() {
                child
            } else {
                format!("{accumulated}/{child}")
            };
            walk_pattern(root, parts, idx + 1, next, manifest, out);
        }
        return;
    }
    let next = if accumulated.is_empty() {
        segment.to_string()
    } else {
        format!("{accumulated}/{segment}")
    };
    walk_pattern(root, parts, idx + 1, next, manifest, out);
}

fn recurse_double_star(
    root: &std::path::Path,
    accumulated: &str,
    remaining: &[&str],
    manifest: &str,
    out: &mut Vec<String>,
    depth: u32,
) {
    // Try matching the remaining pattern at the current level.
    let pseudo_parts: Vec<&str> = remaining.to_vec();
    walk_pattern(root, &pseudo_parts, 0, accumulated.to_string(), manifest, out);
    if depth >= 4 {
        return;
    }
    let dir = root.join(accumulated);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for e in entries.filter_map(std::result::Result::ok) {
        if !e.path().is_dir() {
            continue;
        }
        let child = e.file_name().to_string_lossy().into_owned();
        // Skip common nuisance dirs.
        if matches!(child.as_str(), "node_modules" | "target" | ".git" | ".tokensave") {
            continue;
        }
        let next = if accumulated.is_empty() {
            child
        } else {
            format!("{accumulated}/{child}")
        };
        recurse_double_star(root, &next, remaining, manifest, out, depth + 1);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &std::path::Path, rel: &str) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, "{}").unwrap();
    }

    #[test]
    fn expands_trailing_star() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "crates/a/Cargo.toml");
        write(dir.path(), "crates/b/Cargo.toml");
        write(dir.path(), "crates/c/not-cargo.toml");
        let mut out = expand_workspace_globs(
            dir.path(),
            &["crates/*".to_string()],
            "Cargo.toml",
        );
        out.sort();
        assert_eq!(out, vec!["crates/a", "crates/b"]);
    }

    #[test]
    fn expands_intermediate_star() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "packages/a/bench/package.json");
        write(dir.path(), "packages/b/bench/package.json");
        write(dir.path(), "packages/c/other/package.json");
        let out = expand_workspace_globs(
            dir.path(),
            &["packages/*/bench".to_string()],
            "package.json",
        );
        assert_eq!(out, vec!["packages/a/bench", "packages/b/bench"]);
    }

    #[test]
    fn negation_removes_match() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "packages/a/package.json");
        write(dir.path(), "packages/b/package.json");
        let out = expand_workspace_globs(
            dir.path(),
            &["packages/*".to_string(), "!packages/b".to_string()],
            "package.json",
        );
        assert_eq!(out, vec!["packages/a"]);
    }

    #[test]
    fn double_star_finds_nested_manifests() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "a/Cargo.toml");
        write(dir.path(), "nested/b/Cargo.toml");
        write(dir.path(), "deep/very/c/Cargo.toml");
        let out = expand_workspace_globs(
            dir.path(),
            &["**".to_string()],
            "Cargo.toml",
        );
        // Order is sorted; root itself is "" which won't show up because
        // the recursion starts with empty `accumulated` only at depth 0.
        assert!(out.contains(&"a".to_string()));
        assert!(out.contains(&"nested/b".to_string()));
        assert!(out.contains(&"deep/very/c".to_string()));
    }
}
