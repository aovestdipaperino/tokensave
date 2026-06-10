//! Elixir / Mix ecosystem parser — `mix.exs` (best-effort regex over an
//! Elixir DSL).
//!
//! Captures dep tuples inside `defp deps` / `def deps`:
//!   `{:phoenix, "~> 1.7"}`
//!   `{:plug, "~> 1.14", only: [:dev, :test]}`
//!   `{:my_dep, path: "../my_dep"}`

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "elixir";

pub fn detect(root: &Path) -> bool {
    root.join("mix.exs").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = root.join("mix.exs");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    let app_name = extract_app_name(&raw).unwrap_or_else(|| "mix-project".to_string());
    let deps = extract_dep_tuples(&raw);
    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "mix.exs".to_string(),
            name: app_name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

fn extract_app_name(raw: &str) -> Option<String> {
    let idx = raw.find("app:")?;
    let after = &raw[idx + "app:".len()..];
    // Forms: `app: :my_app` or `app: :"my-app"`.
    let trimmed = after.trim_start();
    if let Some(rest) = trimmed.strip_prefix(':') {
        if let Some(quote_start) = rest.strip_prefix('"') {
            let end = quote_start.find('"')?;
            return Some(quote_start[..end].to_string());
        }
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        return Some(rest[..end].to_string());
    }
    None
}

fn extract_dep_tuples(raw: &str) -> Vec<Dep> {
    let mut out = Vec::new();
    // Each tuple starts with `{:name,` or `{:name }`. Scan for `{:` then
    // walk to the matching `}`.
    let mut pos = 0;
    while let Some(idx) = raw[pos..].find("{:") {
        let start = pos + idx + 1; // skip past `{`
                                   // Walk to matching close brace.
        let inner = &raw[start..];
        let Some(end) = find_matching_brace(inner) else {
            break;
        };
        let body = &inner[..end];
        if let Some(dep) = parse_dep_tuple(body) {
            out.push(dep);
        }
        pos = start + end + 1;
    }
    out
}

fn find_matching_brace(body: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in body.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
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

fn parse_dep_tuple(body: &str) -> Option<Dep> {
    // Body starts with `:name`, optionally followed by `, "~> 1.0"` and
    // keyword options.
    let rest = body.strip_prefix(':')?;
    let name_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    let name = rest[..name_end].to_string();
    if name.is_empty() {
        return None;
    }
    let after = &rest[name_end..];

    let version = first_quoted_after_comma(after);

    let lower = after.to_ascii_lowercase();
    let kind = if lower.contains(":dev") || lower.contains(":test") {
        DepKind::Dev
    } else if lower.contains("only:") && !lower.contains(":prod") {
        // `only: [:dev, :test]` already caught above; `only: :docs` etc. → optional.
        DepKind::Optional
    } else {
        DepKind::Normal
    };

    let local_path = extract_keyword_string(after, "path:");

    Some(Dep {
        name,
        resolved: None,
        version,
        features: Vec::new(),
        optional: matches!(kind, DepKind::Optional),
        local_path,
        kind,
    })
}

fn first_quoted_after_comma(body: &str) -> Option<String> {
    let after_comma = body.find(',').map_or(body, |i| &body[i + 1..]);
    let q = after_comma.find('"')? + 1;
    let rest = &after_comma[q..];
    let e = rest.find('"')?;
    Some(rest[..e].to_string())
}

fn extract_keyword_string(body: &str, key: &str) -> Option<String> {
    let idx = body.find(key)?;
    let after = &body[idx + key.len()..];
    let q = after.find('"')? + 1;
    let rest = &after[q..];
    let e = rest.find('"')?;
    Some(rest[..e].to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_mix_exs() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("mix.exs"),
            r#"
defmodule MyApp.MixProject do
  use Mix.Project
  def project do
    [app: :my_app, version: "0.1.0", deps: deps()]
  end
  defp deps do
    [
      {:phoenix, "~> 1.7"},
      {:plug, "~> 1.14"},
      {:ex_doc, "~> 0.30", only: :dev, runtime: false},
      {:local_lib, path: "../local_lib"},
    ]
  end
end
"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert_eq!(m.name, "my_app");
        let phoenix = m.deps.iter().find(|d| d.name == "phoenix").unwrap();
        assert_eq!(phoenix.version.as_deref(), Some("~> 1.7"));
        let ex_doc = m.deps.iter().find(|d| d.name == "ex_doc").unwrap();
        assert_eq!(ex_doc.kind, DepKind::Dev);
        let local = m.deps.iter().find(|d| d.name == "local_lib").unwrap();
        assert_eq!(local.local_path.as_deref(), Some("../local_lib"));
    }
}
