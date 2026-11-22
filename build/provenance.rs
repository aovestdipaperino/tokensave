//! Source commit provenance for `tokensave --version --verbose` (#1034, #1062).
//!
//! `build.rs` includes this file with `#[path]`, and
//! `tests/build_provenance_test.rs` includes it the same way, so the logic is
//! tested without running a build script. It must therefore only use `std`
//! and the shared names in `build/constants.rs`, which every includer must
//! declare as the sibling module `constants`.
//!
//! Resolution order:
//!
//! 1. A Git checkout (`.git` in the package root): `git rev-parse HEAD`.
//! 2. A packaged crate (crates.io tarball, `cargo package` output): the
//!    `.cargo_vcs_info.json` file that Cargo writes into the archive at
//!    package time, recording the commit the package was built from.
//! 3. Otherwise no commit, reported as `unknown`.
//!
//! Only the package root is inspected, never a parent directory, so a crate
//! unpacked inside an unrelated repository never reports that repository's
//! commit.

use std::path::Path;
use std::process::Command;

use super::constants::{
    git, CARGO_VCS_INFO_FILE_NAME, COMMIT_ABBREV_LEN, COMMIT_MIN_LEN, DIRTY_SUFFIX, FULL_SHA1_LEN,
};

/// Where a commit was found, with the files whose change must rerun the build
/// script.
#[derive(Debug, PartialEq, Eq)]
pub struct Provenance {
    pub commit: Option<String>,
    pub rerun_if_changed: Vec<String>,
}

/// Resolves the commit for the package rooted at `package_root`.
pub fn resolve(package_root: &Path) -> Provenance {
    if package_root.join(git::DIR).exists() {
        return from_git(package_root);
    }
    let vcs_info = package_root.join(CARGO_VCS_INFO_FILE_NAME);
    match std::fs::read_to_string(&vcs_info) {
        Ok(json) => Provenance {
            commit: commit_from_cargo_vcs_info(&json),
            rerun_if_changed: vec![vcs_info.display().to_string()],
        },
        Err(_) => Provenance {
            commit: None,
            rerun_if_changed: Vec::new(),
        },
    }
}

fn from_git(package_root: &Path) -> Provenance {
    let git = |args: &[&str]| command_output(package_root, "git", args);
    let current_ref = git(&["symbolic-ref", "-q", git::HEAD]);
    let rerun_if_changed = [
        Some(git::HEAD),
        Some(git::PACKED_REFS),
        current_ref.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|name| git(&["rev-parse", "--git-path", name]))
    .collect();
    let short = format!("--short={COMMIT_ABBREV_LEN}");
    let commit = git(&["rev-parse", &short, git::HEAD])
        .filter(|sha| sha.len() >= COMMIT_MIN_LEN && is_hex(sha));
    Provenance {
        commit,
        rerun_if_changed,
    }
}

/// Extracts the commit from the contents of `.cargo_vcs_info.json`.
///
/// Cargo writes `{"git": {"sha1": "<40 hex>", "dirty": true}, "path_in_vcs":
/// ""}`, with `dirty` present only when the package was made with
/// `--allow-dirty` from a tree with uncommitted changes. Anything that is not
/// a full SHA-1 yields `None`, so a malformed file reports `unknown` rather
/// than an invented value. A dirty package keeps its commit with
/// [`DIRTY_SUFFIX`], because its sources differ from that commit.
pub fn commit_from_cargo_vcs_info(json: &str) -> Option<String> {
    let sha = json_value_after_key(json, "sha1")?
        .strip_prefix('"')?
        .split('"')
        .next()?;
    if sha.len() != FULL_SHA1_LEN || !is_hex(sha) {
        return None;
    }
    let short = sha.get(..COMMIT_ABBREV_LEN)?.to_ascii_lowercase();
    let dirty = json_value_after_key(json, "dirty").is_some_and(|v| v.starts_with("true"));
    Some(if dirty {
        format!("{short}{DIRTY_SUFFIX}")
    } else {
        short
    })
}

/// Returns the text after `"key":` (whitespace trimmed), or `None` when the
/// key is absent. Sufficient for the flat, Cargo-generated file read here.
fn json_value_after_key<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let quoted = format!("\"{key}\"");
    let (_, rest) = json.split_once(quoted.as_str())?;
    Some(rest.trim_start().strip_prefix(':')?.trim_start())
}

fn is_hex(value: &str) -> bool {
    value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Runs `program` in `dir` and returns its single-line, trimmed stdout.
pub fn command_output(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty() && !value.contains(['\n', '\r'])).then_some(value)
}
