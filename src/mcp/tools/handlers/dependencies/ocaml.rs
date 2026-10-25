//! OCaml ecosystem parser — `*.opam` (opam-format, RFC-822-ish with quoted
//! lists). `dune-project` deps are intentionally deferred (s-expression
//! parsing is more involved and most opam packages also have a `.opam` file).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "ocaml";

pub fn detect(root: &Path) -> bool {
    opam_file(root).is_some() || root.join("dune-project").exists()
}

fn opam_file(root: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut out: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|s| s.to_str()) == Some("opam")).then_some(p)
        })
        .collect();
    out.sort();
    out.into_iter().next()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    if let Some(path) = opam_file(root) {
        return parse_opam(root, &path);
    }
    parse_dune_project(root)
}

fn parse_opam(root: &Path, path: &Path) -> Result<Workspace> {
    let raw = std::fs::read_to_string(path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    let pkg_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("opam-package")
        .to_string();
    let deps = parse_opam_depends(&raw);
    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: path.file_name().map_or_else(
                || "package.opam".to_string(),
                |s| s.to_string_lossy().into_owned(),
            ),
            name: pkg_name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

/// `depends: [ "pkg" {>= "1.0"} "other" {with-test} ... ]`
fn parse_opam_depends(raw: &str) -> Vec<Dep> {
    let mut out = Vec::new();
    let Some(start_idx) = raw.find("depends:") else {
        return out;
    };
    let after = &raw[start_idx + "depends:".len()..];
    let Some(open) = after.find('[') else {
        return out;
    };
    let body = &after[open + 1..];
    let Some(close) = find_matching(body, '[', ']') else {
        return out;
    };
    let body = &body[..close];

    // Walk the body collecting quoted package names, attaching the
    // immediately-following `{ ... }` constraint when present.
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < bytes.len() && bytes[j] != b'"' {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        let name = body[name_start..j].to_string();
        i = j + 1;
        // Skip whitespace before optional `{ ... }`.
        let mut k = i;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        let (version, kind) = if k < bytes.len() && bytes[k] == b'{' {
            let cstart = k + 1;
            let Some(rel_end) = find_matching(&body[cstart..], '{', '}') else {
                break;
            };
            let constraint = body[cstart..cstart + rel_end].to_string();
            i = cstart + rel_end + 1;
            let kind = if constraint.contains("with-test") || constraint.contains("with-dev-setup")
            {
                DepKind::Dev
            } else {
                DepKind::Normal
            };
            (Some(constraint.trim().to_string()), kind)
        } else {
            (None, DepKind::Normal)
        };
        out.push(Dep {
            name,
            resolved: None,
            version,
            features: Vec::new(),
            optional: false,
            local_path: None,
            kind,
        });
    }
    out
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

fn parse_dune_project(root: &Path) -> Result<Workspace> {
    // dune-project uses s-expressions: `(depends pkg1 pkg2 (>= pkg3 1.0))`.
    let path = root.join("dune-project");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    let mut deps = Vec::new();
    // Crude: find every `(depends ` form and extract bare-atom or
    // `(>= name version)` shape.
    let mut pos = 0;
    while let Some(idx) = raw[pos..].find("(depends") {
        let start = pos + idx + "(depends".len();
        let Some(close) = find_matching(&raw[start..], '(', ')') else {
            break;
        };
        let body = &raw[start..start + close];
        for token in body.split_whitespace() {
            let cleaned = token.trim_matches(|c: char| c == '(' || c == ')');
            if cleaned.is_empty() || cleaned.starts_with(':') {
                continue;
            }
            // Skip operators / version literals.
            if matches!(cleaned, ">=" | "<=" | ">" | "<" | "=" | "and" | "or") {
                continue;
            }
            // Numeric → version, skip.
            if cleaned.chars().next().is_some_and(char::is_numeric) {
                continue;
            }
            deps.push(Dep {
                name: cleaned.to_string(),
                resolved: None,
                version: None,
                features: Vec::new(),
                optional: false,
                local_path: None,
                kind: DepKind::Normal,
            });
        }
        pos = start + close + 1;
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "dune-project".to_string(),
            name: "dune-project".to_string(),
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_opam_depends() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("mypkg.opam"),
            r#"opam-version: "2.0"
synopsis: "Example"
depends: [
  "dune" {>= "3.0"}
  "ocaml" {>= "4.14"}
  "alcotest" {with-test}
]
"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        let dune = m.deps.iter().find(|d| d.name == "dune").unwrap();
        assert_eq!(dune.version.as_deref(), Some(">= \"3.0\""));
        let alcotest = m.deps.iter().find(|d| d.name == "alcotest").unwrap();
        assert_eq!(alcotest.kind, DepKind::Dev);
    }
}
