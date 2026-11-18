use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::mcp::fs_watch::MAX_PENDING_PATHS;

#[derive(Default)]
pub(super) struct IgnoreScope {
    directories: HashMap<PathBuf, Rules>,
    global: Option<Gitignore>,
    pub(super) exhausted: bool,
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
        ignore
            .or(exclude.filter(|_| repository))
            .unwrap_or_else(|| {
                !repository
                    || !self
                        .global
                        .get_or_insert_with(|| Gitignore::global().0)
                        .matched(canonical_path, is_dir)
                        .is_ignore()
            })
    }

    fn rules(&mut self, dir: &Path) -> Option<&Rules> {
        if self.directories.len() == MAX_PENDING_PATHS && !self.directories.contains_key(dir) {
            self.exhausted = true;
            return None;
        }
        Some(
            self.directories
                .entry(dir.to_path_buf())
                .or_insert_with(|| Rules::read(dir)),
        )
    }
}

impl Rules {
    fn read(dir: &Path) -> Self {
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
        let exclude = common_dir.map_or_else(Gitignore::empty, |common| {
            read_rules(dir, &common.join("info").join("exclude"))
        });
        Self {
            gitignore: read_rules(dir, &dir.join(".gitignore")),
            ignore: read_rules(dir, &dir.join(".ignore")),
            exclude,
            repository,
            canonical_path: dir.canonicalize().ok(),
        }
    }
}

fn read_rules(dir: &Path, path: &Path) -> Gitignore {
    if !path.is_file() {
        return Gitignore::empty();
    }
    let mut builder = GitignoreBuilder::new(dir);
    let _ = builder.add(path);
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

fn decision(rules: &Gitignore, candidate: &Path, is_dir: bool) -> Option<bool> {
    let result = rules.matched(candidate, is_dir);
    (!result.is_none()).then_some(!result.is_ignore())
}
