//! Ruby ecosystem parser — `Gemfile`.
//!
//! Gemfile is a Ruby DSL; we don't attempt to execute or fully parse it.
//! Instead we extract the common shape:
//!   `gem 'name', 'version', :group => :dev`
//!   `gem "name"`
//!   `group :development do ... gem 'rspec' ... end`

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "ruby";

pub fn detect(root: &Path) -> bool {
    root.join("Gemfile").exists() || root.join("gems.rb").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let manifest = if root.join("Gemfile").exists() {
        root.join("Gemfile")
    } else {
        root.join("gems.rb")
    };
    let raw = std::fs::read_to_string(&manifest).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", manifest.display()),
    })?;

    let mut deps: Vec<Dep> = Vec::new();
    let mut group_stack: Vec<DepKind> = Vec::new();

    for raw_line in raw.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        // `group :development do` opens a scope; `end` closes it.
        if let Some(rest) = line.strip_prefix("group ") {
            let is_devlike =
                rest.contains(":development") || rest.contains(":dev") || rest.contains(":test");
            let kind = if is_devlike {
                DepKind::Dev
            } else if rest.contains(":production") {
                DepKind::Normal
            } else {
                DepKind::Optional
            };
            group_stack.push(kind);
            continue;
        }
        if line == "end" || line.starts_with("end ") {
            group_stack.pop();
            continue;
        }

        if let Some(dep) = parse_gem_line(line, group_stack.last().copied()) {
            deps.push(dep);
        }
    }

    let member = Member {
        path: manifest.file_name().map_or_else(
            || "Gemfile".to_string(),
            |s| s.to_string_lossy().into_owned(),
        ),
        name: root.file_name().map_or_else(
            || "ruby-app".to_string(),
            |s| s.to_string_lossy().into_owned(),
        ),
        license: None,
        deps,
    };

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![member],
        patches: Vec::new(),
    })
}

fn parse_gem_line(line: &str, scope: Option<DepKind>) -> Option<Dep> {
    let rest = line.strip_prefix("gem ")?;
    // Pull out quoted tokens in order: first = name, second = version (optional).
    let mut iter = QuoteIter::new(rest);
    let name = iter.next()?.to_string();
    let version = iter.next().map(str::to_string);

    // Inline group hints: `:group => :dev` or `group: :dev`.
    let lower = line.to_ascii_lowercase();
    let inline_dev = lower.contains(":development")
        || lower.contains("=> :dev")
        || lower.contains("group: :dev")
        || lower.contains(":test");
    let inline_kind = inline_dev.then_some(DepKind::Dev);

    let kind = inline_kind.or(scope).unwrap_or(DepKind::Normal);

    let local_path = if let Some(idx) = lower.find("path:") {
        extract_after(line, idx + "path:".len())
    } else if let Some(idx) = lower.find(":path =>") {
        extract_after(line, idx + ":path =>".len())
    } else {
        None
    };

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

/// Tiny iterator over single/double-quoted substrings in a line.
struct QuoteIter<'a> {
    rest: &'a str,
}
impl<'a> QuoteIter<'a> {
    fn new(s: &'a str) -> Self {
        Self { rest: s }
    }
}
impl<'a> Iterator for QuoteIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            if c == b'\'' || c == b'"' {
                let quote = c;
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != quote {
                    j += 1;
                }
                if j >= bytes.len() {
                    return None;
                }
                let s = &self.rest[start..j];
                self.rest = &self.rest[j + 1..];
                return Some(s);
            }
            i += 1;
        }
        None
    }
}

fn extract_after(line: &str, idx: usize) -> Option<String> {
    let mut iter = QuoteIter::new(&line[idx..]);
    iter.next().map(str::to_string)
}

fn strip_comment(line: &str) -> &str {
    if let Some(idx) = line.find('#') {
        // Avoid breaking on `#{ruby_interp}` — but Gemfiles rarely use that
        // in dep declarations. Cheap heuristic: only strip when `#` is
        // preceded by whitespace.
        if idx == 0 || line.as_bytes()[idx - 1].is_ascii_whitespace() {
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

    #[test]
    fn parses_simple_gemfile() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Gemfile"),
            r#"
source 'https://rubygems.org'

gem 'rails', '~> 7.1'
gem 'pg'

group :development, :test do
  gem 'rspec', '3.12'
  gem 'pry'
end

group :production do
  gem 'puma'
end
"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "ruby");
        let m = &ws.members[0];
        let rails = m.deps.iter().find(|d| d.name == "rails").unwrap();
        assert_eq!(rails.version.as_deref(), Some("~> 7.1"));
        assert_eq!(rails.kind, DepKind::Normal);
        let rspec = m.deps.iter().find(|d| d.name == "rspec").unwrap();
        assert_eq!(rspec.kind, DepKind::Dev);
        let puma = m.deps.iter().find(|d| d.name == "puma").unwrap();
        assert_eq!(puma.kind, DepKind::Normal);
    }

    #[test]
    fn parses_inline_group() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Gemfile"),
            "gem 'rubocop', group: :development\n",
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let dep = ws.members[0]
            .deps
            .iter()
            .find(|d| d.name == "rubocop")
            .unwrap();
        assert_eq!(dep.kind, DepKind::Dev);
    }
}
