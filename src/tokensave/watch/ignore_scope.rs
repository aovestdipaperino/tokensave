use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};

use ignore::gitignore::{gitconfig_excludes_path, Gitignore, GitignoreBuilder};

use crate::mcp::fs_watch::MAX_PENDING_PATHS;

const MAX_RULE_BYTES: usize = 1024 * 1024;

pub(super) struct IgnoreScope {
    directories: HashMap<PathBuf, Rules>,
    global: Option<Gitignore>,
    remaining_rule_bytes: usize,
    pub(super) exhausted: bool,
}

impl Default for IgnoreScope {
    fn default() -> Self {
        Self {
            directories: HashMap::new(),
            global: None,
            remaining_rule_bytes: MAX_RULE_BYTES,
            exhausted: false,
        }
    }
}

struct Rules {
    gitignore: Gitignore,
    ignore: Gitignore,
    exclude: Gitignore,
    repository: bool,
    canonical_path: Option<PathBuf>,
}

impl IgnoreScope {
    pub(super) fn allows(&mut self, path: &Path, is_dir: bool) -> bool {
        let Some((parent, name)) = path.parent().zip(path.file_name()) else {
            return false;
        };
        let Some(local) = self.rules(parent) else {
            return false;
        };
        let Some(canonical_parent) = local.canonical_path.clone() else {
            return false;
        };
        let canonical_path = canonical_parent.join(name);
        let ancestors = canonical_parent.ancestors().skip(1);
        let mut ignore = None;
        let mut exclude = None;
        let mut repository = false;
        for (dir, candidate) in std::iter::once((parent, path))
            .chain(ancestors.map(|dir| (dir, canonical_path.as_path())))
        {
            let Some(rules) = self.rules(dir) else {
                return false;
            };
            // The scan treats .gitignore as a custom ignore file, ahead of .ignore.
            if let Some(allowed) = decision(&rules.gitignore, candidate, is_dir) {
                return allowed;
            }
            ignore = ignore.or_else(|| decision(&rules.ignore, candidate, is_dir));
            if !repository {
                exclude = exclude.or_else(|| decision(&rules.exclude, candidate, is_dir));
                repository = rules.repository;
            }
        }
        if let Some(allowed) = ignore.or(exclude.filter(|_| repository)) {
            return allowed;
        }
        !repository
            || self
                .global_rules()
                .is_some_and(|rules| !rules.matched(canonical_path, is_dir).is_ignore())
    }

    fn rules(&mut self, dir: &Path) -> Option<&Rules> {
        if !self.directories.contains_key(dir) {
            if self.exhausted || self.directories.len() == MAX_PENDING_PATHS {
                self.exhausted = true;
                return None;
            }
            let Some(rules) = Rules::read(dir, &mut self.remaining_rule_bytes) else {
                self.exhausted = true;
                return None;
            };
            self.directories.insert(dir.to_path_buf(), rules);
        }
        self.directories.get(dir)
    }

    fn global_rules(&mut self) -> Option<&Gitignore> {
        if self.global.is_none() && !self.exhausted {
            let source = std::env::current_dir()
                .ok()
                .and_then(|cwd| gitconfig_excludes_path().map(|path| (cwd, path)));
            self.global = match source {
                Some((cwd, path)) => read_rules(&cwd, &path, &mut self.remaining_rule_bytes),
                None => Some(Gitignore::empty()),
            };
            self.exhausted = self.global.is_none();
        }
        self.global.as_ref()
    }
}

impl Rules {
    fn read(dir: &Path, remaining: &mut usize) -> Option<Self> {
        let git = dir.join(".git");
        let git_type = git.metadata().ok().map(|metadata| metadata.file_type());
        let repository = git_type.is_some() || dir.join(".jj").exists();
        let common_dir = if git_type.is_some_and(|kind| kind.is_file()) {
            gix::open(dir)
                .ok()
                .map(|repo| repo.common_dir().to_path_buf())
        } else {
            Some(git)
        };
        let exclude = match common_dir {
            Some(common) => read_rules(dir, &common.join("info").join("exclude"), remaining)?,
            None => Gitignore::empty(),
        };
        Some(Self {
            gitignore: read_rules(dir, &dir.join(".gitignore"), remaining)?,
            ignore: read_rules(dir, &dir.join(".ignore"), remaining)?,
            exclude,
            repository,
            canonical_path: dir.canonicalize().ok(),
        })
    }
}

fn read_rules(dir: &Path, path: &Path, remaining: &mut usize) -> Option<Gitignore> {
    if !path.is_file() {
        return Some(Gitignore::empty());
    }
    let Ok(file) = File::open(path) else {
        return Some(Gitignore::empty());
    };
    // The extra byte distinguishes an exact fit from an incomplete rule file.
    let mut bytes = Vec::new();
    let read = file.take(*remaining as u64 + 1).read_to_end(&mut bytes);
    *remaining = remaining.checked_sub(bytes.len())?;
    if read.is_err() {
        let complete_lines = bytes
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(0, |index| index + 1);
        bytes.truncate(complete_lines);
    }

    let mut builder = GitignoreBuilder::new(dir);
    for (index, line) in bytes.as_slice().lines().enumerate() {
        let Ok(line) = line else { break };
        let rule = if index == 0 {
            line.trim_start_matches('\u{feff}')
        } else {
            &line
        };
        let _ = builder.add_line(Some(path.to_path_buf()), rule);
    }
    Some(builder.build().unwrap_or_else(|_| Gitignore::empty()))
}

fn decision(rules: &Gitignore, candidate: &Path, is_dir: bool) -> Option<bool> {
    let result = rules.matched(candidate, is_dir);
    (!result.is_none()).then_some(!result.is_ignore())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;

    use super::{read_rules, IgnoreScope};

    #[test]
    fn ignore_rule_budget_is_shared_and_rejects_partial_files() -> io::Result<()> {
        let fixture = tempfile::tempdir()?;
        let root = fixture.path();
        assert!(gix::init(root).is_ok());
        let child = root.join("child");
        fs::create_dir(&child)?;

        let local = b"skip.rs\n";
        let exclude = b"omit.rs\n";
        let info = root.join(".git").join("info");
        fs::create_dir_all(&info)?;
        fs::write(info.join("exclude"), exclude)?;
        let root_rules = b"\xef\xbb\xbf!keep.rs\r\n#";
        let mut padded = root_rules.to_vec();
        padded.resize(1024 * 1024 - local.len() - exclude.len(), b'x');
        fs::write(root.join(".gitignore"), padded)?;
        fs::write(root.join(".ignore"), local)?;
        fs::write(child.join(".gitignore"), b"!keep.rs\nkeep.rs\n")?;

        let mut scope = IgnoreScope::default();
        assert!(scope.allows(&root.join("keep.rs"), false));
        assert!(!scope.exhausted);
        assert!(scope.allows(&root.join("keep.rs"), false));
        assert!(!scope.allows(&child.join("keep.rs"), false));
        assert!(
            scope.exhausted,
            "the next rule file exceeds the shared budget"
        );
        assert!(!scope.allows(&child.join("keep.rs"), false));
        assert!(scope.allows(&root.join("keep.rs"), false));
        Ok(())
    }

    #[test]
    fn ignore_rule_budget_does_not_compile_a_truncated_prefix() -> io::Result<()> {
        let fixture = tempfile::tempdir()?;
        let root = fixture.path();
        let rules = root.join(".gitignore");
        fs::write(&rules, b"!keep.rs\nkeep.rs\n")?;
        let mut remaining = b"!keep.rs\n".len();

        assert!(read_rules(root, &rules, &mut remaining).is_none());
        Ok(())
    }
}
