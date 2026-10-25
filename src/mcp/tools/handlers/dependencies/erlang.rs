//! Erlang / rebar3 ecosystem parser — `rebar.config` (Erlang terms,
//! best-effort regex).
//!
//! Captures the common `{deps, [{name, "1.0"}, {name, {git, "url", {tag, "v1"}}}, ...]}`.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "erlang";

pub fn detect(root: &Path) -> bool {
    root.join("rebar.config").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = root.join("rebar.config");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;

    let mut deps = collect_section_deps(&raw, "deps", DepKind::Normal);
    deps.extend(collect_section_deps(&raw, "profiles", DepKind::Dev));

    let app_name = root.file_name().map_or_else(
        || "rebar-app".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "rebar.config".to_string(),
            name: app_name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

fn collect_section_deps(raw: &str, key: &str, kind: DepKind) -> Vec<Dep> {
    // Find `{key, [` and walk to the matching `]`.
    let pat = format!("{{{key}, [");
    let Some(start) = raw.find(&pat) else {
        return Vec::new();
    };
    let body_start = start + pat.len();
    let Some(end) = find_matching(&raw[body_start..], '[', ']') else {
        return Vec::new();
    };
    let body = &raw[body_start..body_start + end];

    // Each dep entry is `{name, ...}`. Extract them by walking braces.
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(idx) = body[pos..].find('{') {
        let inner_start = pos + idx + 1;
        let Some(close) = find_matching(&body[inner_start..], '{', '}') else {
            break;
        };
        let inner = &body[inner_start..inner_start + close];
        if let Some(dep) = parse_entry(inner, kind) {
            out.push(dep);
        }
        pos = inner_start + close + 1;
    }
    out
}

fn parse_entry(inner: &str, kind: DepKind) -> Option<Dep> {
    // First atom = dep name.
    let trimmed = inner.trim_start();
    let name_end = trimmed
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    let name = trimmed[..name_end].to_string();
    if name.is_empty() {
        return None;
    }
    // First quoted string after comma = version (for hex deps).
    let after = &trimmed[name_end..];
    let version = first_quoted_after_comma(after);
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

fn first_quoted_after_comma(body: &str) -> Option<String> {
    let after = body.find(',').map_or(body, |i| &body[i + 1..]);
    let q = after.find('"')? + 1;
    let rest = &after[q..];
    let e = rest.find('"')?;
    Some(rest[..e].to_string())
}

fn find_matching(body: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in body.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
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
    fn parses_rebar_config() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("rebar.config"),
            r#"
{erl_opts, [debug_info]}.
{deps, [
    {cowboy, "2.10.0"},
    {jsx, "3.1.0"}
]}.
"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        let cowboy = m.deps.iter().find(|d| d.name == "cowboy").unwrap();
        assert_eq!(cowboy.version.as_deref(), Some("2.10.0"));
        let jsx = m.deps.iter().find(|d| d.name == "jsx").unwrap();
        assert_eq!(jsx.version.as_deref(), Some("3.1.0"));
    }
}
