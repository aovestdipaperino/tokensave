//! Python ecosystem parser — `pyproject.toml` (PEP 621 + Poetry) +
//! `requirements*.txt` (pip).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "python";

pub fn detect(root: &Path) -> bool {
    root.join("pyproject.toml").exists()
        || root.join("requirements.txt").exists()
        || root.join("setup.py").exists()
        || root.join("Pipfile").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let mut members: Vec<Member> = Vec::new();

    if root.join("pyproject.toml").exists() {
        if let Some(m) = parse_pyproject(root) {
            members.push(m);
        }
    }

    // requirements*.txt — one member per file, since pip projects often have
    // requirements.txt, requirements-dev.txt, etc.
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut req_files: Vec<String> = entries
            .filter_map(std::result::Result::ok)
            .filter_map(|e| {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                let starts = name.to_ascii_lowercase().starts_with("requirements");
                let ext_ok = matches!(
                    path.extension()
                        .and_then(|s| s.to_str())
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some("txt" | "in")
                );
                (starts && ext_ok).then_some(name)
            })
            .collect();
        req_files.sort();
        for req in req_files {
            if let Some(m) = parse_requirements(root, &req) {
                members.push(m);
            }
        }
    }

    if members.is_empty() {
        return Err(TokenSaveError::Config {
            message: format!(
                "no Python manifest found at {} (looked for pyproject.toml, requirements*.txt)",
                root.display()
            ),
        });
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members,
        patches: Vec::new(),
    })
}

fn parse_pyproject(root: &Path) -> Option<Member> {
    let path = root.join("pyproject.toml");
    let raw = std::fs::read_to_string(&path).ok()?;
    let doc: toml::Value = toml::from_str(&raw).ok()?;

    let project_name = doc
        .get("project")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("name"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            doc.get("tool")
                .and_then(|v| v.get("poetry"))
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("name"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("pyproject")
        .to_string();

    // PEP 621 license: either `license = "MIT"` or `license = { text = "..." }`
    // or `license = { file = "LICENSE" }`. Poetry uses `[tool.poetry] license`.
    let license = doc
        .get("project")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("license"))
        .and_then(|v| match v {
            toml::Value::String(s) => Some(s.clone()),
            toml::Value::Table(t) => t
                .get("text")
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .or_else(|| {
                    t.get("file")
                        .and_then(|x| x.as_str())
                        .map(|p| format!("file:{p}"))
                }),
            _ => None,
        })
        .or_else(|| {
            doc.get("tool")
                .and_then(|v| v.get("poetry"))
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("license"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });

    let mut deps: Vec<Dep> = Vec::new();

    // PEP 621: [project.dependencies] is a list of PEP 508 strings.
    if let Some(arr) = doc
        .get("project")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("dependencies"))
        .and_then(|v| v.as_array())
    {
        for v in arr {
            if let Some(s) = v.as_str() {
                deps.push(pep508_to_dep(s, DepKind::Normal));
            }
        }
    }

    // PEP 621: [project.optional-dependencies] = { dev = ["pytest"], ... }
    if let Some(tbl) = doc
        .get("project")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("optional-dependencies"))
        .and_then(|v| v.as_table())
    {
        for (group_name, value) in tbl {
            let Some(arr) = value.as_array() else {
                continue;
            };
            // "dev"/"test" → DepKind::Dev; everything else → optional.
            let kind = if matches!(group_name.as_str(), "dev" | "test" | "tests" | "testing") {
                DepKind::Dev
            } else {
                DepKind::Optional
            };
            for v in arr {
                if let Some(s) = v.as_str() {
                    deps.push(pep508_to_dep(s, kind));
                }
            }
        }
    }

    // Poetry: [tool.poetry.dependencies] / [tool.poetry.group.dev.dependencies]
    if let Some(tbl) = doc
        .get("tool")
        .and_then(|v| v.get("poetry"))
        .and_then(|v| v.get("dependencies"))
        .and_then(|v| v.as_table())
    {
        for (name, value) in tbl {
            if name == "python" {
                continue;
            }
            deps.push(poetry_dep(name, value, DepKind::Normal));
        }
    }
    if let Some(groups) = doc
        .get("tool")
        .and_then(|v| v.get("poetry"))
        .and_then(|v| v.get("group"))
        .and_then(|v| v.as_table())
    {
        for (group_name, group_tbl) in groups {
            let kind = if matches!(group_name.as_str(), "dev" | "test" | "tests") {
                DepKind::Dev
            } else {
                DepKind::Optional
            };
            let Some(deps_tbl) = group_tbl
                .as_table()
                .and_then(|t| t.get("dependencies"))
                .and_then(|v| v.as_table())
            else {
                continue;
            };
            for (name, value) in deps_tbl {
                deps.push(poetry_dep(name, value, kind));
            }
        }
    }

    Some(Member {
        path: "pyproject.toml".to_string(),
        name: project_name,
        license,
        deps,
    })
}

fn poetry_dep(name: &str, value: &toml::Value, kind: DepKind) -> Dep {
    match value {
        toml::Value::String(s) => Dep {
            name: name.to_string(),
            resolved: None,
            version: Some(s.clone()),
            features: Vec::new(),
            optional: matches!(kind, DepKind::Optional),
            local_path: None,
            kind,
        },
        toml::Value::Table(t) => {
            let version = t
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let optional = t
                .get("optional")
                .and_then(toml::Value::as_bool)
                .unwrap_or(matches!(kind, DepKind::Optional));
            let path = t
                .get("path")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let extras = t
                .get("extras")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Dep {
                name: name.to_string(),
                resolved: None,
                version,
                features: extras,
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

/// Best-effort PEP 508 string parse: `name[extras] (operator)version`.
/// We just split on the first non-identifier character to get the name and
/// take the rest as the version expression.
fn pep508_to_dep(spec: &str, kind: DepKind) -> Dep {
    let spec = spec.split(';').next().unwrap_or(spec).trim();
    let mut name_end = spec.len();
    for (i, c) in spec.char_indices() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            name_end = i;
            break;
        }
    }
    let name = spec[..name_end].to_string();
    let rest = spec[name_end..].trim();

    let mut features = Vec::new();
    let version_part = if let Some(b_open) = rest.find('[') {
        if let Some(b_close) = rest[b_open..].find(']') {
            features = rest[b_open + 1..b_open + b_close]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            rest[b_open + b_close + 1..].trim()
        } else {
            rest
        }
    } else {
        rest
    };
    let version = if version_part.is_empty() {
        None
    } else {
        Some(version_part.to_string())
    };

    Dep {
        name,
        resolved: None,
        version,
        features,
        optional: matches!(kind, DepKind::Optional),
        local_path: None,
        kind,
    }
}

fn parse_requirements(root: &Path, filename: &str) -> Option<Member> {
    let path = root.join(filename);
    let raw = std::fs::read_to_string(&path).ok()?;
    let kind = if filename.contains("dev") || filename.contains("test") {
        DepKind::Dev
    } else {
        DepKind::Normal
    };
    let mut deps = Vec::new();
    for line in raw.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        // Skip pip directives we can't usefully attribute (-r, -c, --index-url, etc.)
        if line.starts_with('-') {
            continue;
        }
        deps.push(pep508_to_dep(line, kind));
    }
    Some(Member {
        path: filename.to_string(),
        name: filename.to_string(),
        license: None,
        deps,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, content: &str) {
        fs::write(root.join(rel), content).unwrap();
    }

    #[test]
    fn parses_pep621_project_dependencies() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "pyproject.toml",
            r#"
[project]
name = "my-pkg"
version = "0.1.0"
dependencies = ["requests>=2.0", "click==8.1.7"]

[project.optional-dependencies]
dev = ["pytest>=7", "ruff"]
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = ws.members.iter().find(|m| m.name == "my-pkg").unwrap();
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "requests" && d.kind == DepKind::Normal));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "pytest" && d.kind == DepKind::Dev));
    }

    #[test]
    fn parses_poetry_dependencies() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "pyproject.toml",
            r#"
[tool.poetry]
name = "poetry-pkg"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.10"
requests = "^2.31"
django = { version = "^5.0", extras = ["postgres"] }

[tool.poetry.group.dev.dependencies]
pytest = "^7.0"
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = ws.members.iter().find(|m| m.name == "poetry-pkg").unwrap();
        // `python` is filtered out — it's a marker, not a dep.
        assert!(!m.deps.iter().any(|d| d.name == "python"));
        let django = m.deps.iter().find(|d| d.name == "django").unwrap();
        assert!(django.features.contains(&"postgres".to_string()));
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "pytest" && d.kind == DepKind::Dev));
    }

    #[test]
    fn parses_requirements_txt() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "requirements.txt",
            "
# pinned for security
requests==2.31.0
click>=8.0,<9
flask  # web framework
-r constraints.txt
",
        );
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert_eq!(m.name, "requirements.txt");
        assert!(m.deps.iter().any(|d| d.name == "requests"));
        assert!(m.deps.iter().any(|d| d.name == "click"));
        assert!(m.deps.iter().any(|d| d.name == "flask"));
    }

    #[test]
    fn classifies_dev_requirements_file() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "requirements.txt", "requests==2.31.0\n");
        write(dir.path(), "requirements-dev.txt", "pytest>=7\n");
        let ws = parse(dir.path()).unwrap();
        let dev = ws
            .members
            .iter()
            .find(|m| m.path == "requirements-dev.txt")
            .unwrap();
        assert!(dev.deps.iter().all(|d| d.kind == DepKind::Dev));
    }

    #[test]
    fn pep508_parses_extras_and_version() {
        let d = pep508_to_dep("requests[security] >= 2.31 ; python_version >= '3.7'", DepKind::Normal);
        assert_eq!(d.name, "requests");
        assert_eq!(d.features, vec!["security".to_string()]);
        assert!(d.version.unwrap().contains("2.31"));
    }
}
