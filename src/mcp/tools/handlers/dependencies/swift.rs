//! Swift / `SwiftPM` ecosystem parser — `Package.swift` (best-effort regex)
//! and `Package.resolved` lockfile.
//!
//! `Package.swift` is real Swift source — we don't evaluate it. The
//! parser captures the common `.package(url: "...", from: "...")` and
//! `.package(url: "...", .upToNextMajor(from: "..."))` shapes.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "swift";

pub fn detect(root: &Path) -> bool {
    root.join("Package.swift").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = root.join("Package.swift");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;

    let name = extract_package_name(&raw).unwrap_or_else(|| "swift-package".to_string());
    let deps = extract_dependencies(&raw);

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "Package.swift".to_string(),
            name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

fn extract_package_name(raw: &str) -> Option<String> {
    // `Package(name: "Foo", ...)` — first `name:` is the package's own name.
    let idx = raw.find("name:")?;
    let after = &raw[idx + "name:".len()..];
    let quote_start = after.find('"')? + 1;
    let rest = &after[quote_start..];
    let quote_end = rest.find('"')?;
    Some(rest[..quote_end].to_string())
}

fn extract_dependencies(raw: &str) -> Vec<Dep> {
    let mut out = Vec::new();
    // Find every `.package(` token, then walk its parenthesized body.
    let mut search_pos = 0;
    while let Some(idx) = raw[search_pos..].find(".package(") {
        let start = search_pos + idx + ".package(".len();
        let Some(end) = find_matching_paren(&raw[start..]) else {
            break;
        };
        let body = &raw[start..start + end];
        if let Some(dep) = parse_package_call(body) {
            out.push(dep);
        }
        search_pos = start + end + 1;
    }
    out
}

/// Given the contents of `.package( ... )`, pull out URL/name and version.
fn parse_package_call(body: &str) -> Option<Dep> {
    // Identify the dep name: either url's last path component or `name:` arg.
    let url = extract_string_arg(body, "url:").or_else(|| extract_string_arg(body, "path:"));
    let name_arg = extract_string_arg(body, "name:");

    let name = name_arg
        .clone()
        .or_else(|| url.as_ref().map(|u| extract_repo_name(u)))?;

    // Version specs: `from: "1.0.0"`, `exact: "1.0.0"`, `branch: "main"`,
    // `revision: "abc123"`, `.upToNextMajor(from: "1.0.0")`,
    // `.upToNextMinor(from: "1.0.0")`, ranges `"1.0.0"..<"2.0.0"`.
    let version = extract_string_arg(body, "from:")
        .or_else(|| extract_string_arg(body, "exact:"))
        .or_else(|| extract_string_arg(body, "branch:"))
        .or_else(|| extract_string_arg(body, "revision:"))
        .or_else(|| {
            // Pull the first quoted string anywhere in the body as a last
            // resort (catches `"1.0.0"..<"2.0.0"`-style ranges).
            extract_first_quoted(body).filter(|s| s != url.as_deref().unwrap_or(""))
        });

    let local_path = if body.contains("path:") {
        url.clone()
    } else {
        None
    };

    Some(Dep {
        name,
        resolved: None,
        version,
        features: Vec::new(),
        optional: false,
        local_path,
        kind: DepKind::Normal,
    })
}

fn extract_string_arg(body: &str, key: &str) -> Option<String> {
    let idx = body.find(key)?;
    let after = &body[idx + key.len()..];
    let quote_start = after.find('"')? + 1;
    let rest = &after[quote_start..];
    let quote_end = rest.find('"')?;
    Some(rest[..quote_end].to_string())
}

fn extract_first_quoted(body: &str) -> Option<String> {
    let start = body.find('"')? + 1;
    let rest = &body[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_repo_name(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit('/').next().unwrap_or(trimmed);
    last.strip_suffix(".git").unwrap_or(last).to_string()
}

fn find_matching_paren(body: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in body.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_package_swift_basic() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Package.swift"),
            r#"
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MyLib",
    dependencies: [
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.0.0"),
        .package(url: "https://github.com/apple/swift-log", .upToNextMajor(from: "1.0.0")),
        .package(name: "LocalLib", path: "../LocalLib"),
    ]
)
"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "swift");
        let m = &ws.members[0];
        assert_eq!(m.name, "MyLib");
        let nio = m.deps.iter().find(|d| d.name == "swift-nio").unwrap();
        assert_eq!(nio.version.as_deref(), Some("2.0.0"));
        assert!(m.deps.iter().any(|d| d.name == "swift-log"));
        let local = m.deps.iter().find(|d| d.name == "LocalLib").unwrap();
        assert!(local.local_path.is_some());
    }

    #[test]
    fn extracts_repo_name_strips_git_suffix() {
        assert_eq!(extract_repo_name("https://github.com/foo/bar.git"), "bar");
        assert_eq!(extract_repo_name("https://github.com/foo/bar"), "bar");
        assert_eq!(extract_repo_name("https://github.com/foo/bar/"), "bar");
    }
}
