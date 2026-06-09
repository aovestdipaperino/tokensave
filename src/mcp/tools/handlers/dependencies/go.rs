//! Go ecosystem parser — `go.mod`.
//!
//! `go.mod` is line-oriented with `require`, `replace`, `exclude` directives.
//! We parse the directives we care about; comments and indirect markers are
//! preserved as features for transparency.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Patch, Workspace};

const ECOSYSTEM: &str = "go";

pub fn detect(root: &Path) -> bool {
    root.join("go.mod").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let manifest = root.join("go.mod");
    let raw = std::fs::read_to_string(&manifest).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", manifest.display()),
    })?;

    let module = extract_module(&raw)
        .unwrap_or_else(|| root.file_name().map_or_else(|| ".".into(), |s| s.to_string_lossy().to_string()));
    let deps = collect_requires(&raw);
    let patches = collect_replaces(&raw);

    let member = Member {
        path: "go.mod".to_string(),
        name: module,
        deps,
    };

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![member],
        patches,
    })
}

fn extract_module(src: &str) -> Option<String> {
    for line in src.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("module ") {
            return Some(rest.split_whitespace().next()?.trim_matches('"').to_string());
        }
    }
    None
}

fn collect_requires(src: &str) -> Vec<Dep> {
    let mut out = Vec::new();
    let mut in_block = false;
    for raw_line in src.lines() {
        let line = strip_comment(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if in_block {
            if trimmed.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(dep) = parse_require_line(trimmed, raw_line) {
                out.push(dep);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("require") {
            let rest = rest.trim();
            if rest.starts_with('(') {
                in_block = true;
                continue;
            }
            // Single-line require: `require foo/bar v1.2.3`.
            if let Some(dep) = parse_require_line(rest, raw_line) {
                out.push(dep);
            }
        }
    }
    out
}

fn parse_require_line(content: &str, raw_line: &str) -> Option<Dep> {
    let mut tokens = content.split_whitespace();
    let name = tokens.next()?.to_string();
    let version = tokens.next()?.to_string();
    // The `// indirect` comment on the raw line lets us distinguish indirect
    // deps without changing kind — surface it as a feature for visibility.
    let mut features = Vec::new();
    if raw_line.contains("// indirect") {
        features.push("indirect".to_string());
    }
    Some(Dep {
        name,
        version: Some(version),
        features,
        optional: false,
        local_path: None,
        kind: DepKind::Normal,
    })
}

fn collect_replaces(src: &str) -> Vec<Patch> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in src.lines() {
        let line = strip_comment(line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if in_block {
            if trimmed.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(p) = parse_replace_line(trimmed) {
                out.push(p);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("replace") {
            let rest = rest.trim();
            if rest.starts_with('(') {
                in_block = true;
                continue;
            }
            if let Some(p) = parse_replace_line(rest) {
                out.push(p);
            }
        }
    }
    out
}

/// `replace foo => ../local`  OR  `replace foo v1 => bar v2`.
fn parse_replace_line(content: &str) -> Option<Patch> {
    let mut parts = content.splitn(2, "=>");
    let lhs = parts.next()?.trim();
    let rhs = parts.next()?.trim();
    let name = lhs.split_whitespace().next()?.to_string();
    Some(Patch {
        source: "go-replace".to_string(),
        name,
        replacement: rhs.to_string(),
    })
}

fn strip_comment(line: &str) -> &str {
    if let Some(idx) = line.find("//") {
        // Preserve `// indirect` markers — only strip *other* comments.
        if !line[idx..].trim_start().starts_with("// indirect") {
            return &line[..idx];
        }
    }
    line
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, content: &str) {
        fs::write(root.join("go.mod"), content).unwrap();
    }

    #[test]
    fn parses_require_block() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            r#"
module github.com/foo/bar

go 1.22

require (
    github.com/spf13/cobra v1.8.0
    github.com/sirupsen/logrus v1.9.0 // indirect
)
"#,
        );
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "go");
        let m = &ws.members[0];
        assert_eq!(m.name, "github.com/foo/bar");
        let cobra = m.deps.iter().find(|d| d.name == "github.com/spf13/cobra").unwrap();
        assert_eq!(cobra.version.as_deref(), Some("v1.8.0"));
        let logrus = m.deps.iter().find(|d| d.name == "github.com/sirupsen/logrus").unwrap();
        assert!(logrus.features.contains(&"indirect".to_string()));
    }

    #[test]
    fn parses_single_line_require() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            r#"
module example.com/x
go 1.22
require example.com/y v0.1.0
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert!(m.deps.iter().any(|d| d.name == "example.com/y"));
    }

    #[test]
    fn parses_replace_directive_as_patch() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            r#"
module example.com/x
go 1.22

replace example.com/y => ../local-y
"#,
        );
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.patches.len(), 1);
        assert_eq!(ws.patches[0].name, "example.com/y");
        assert!(ws.patches[0].replacement.contains("../local-y"));
    }
}
