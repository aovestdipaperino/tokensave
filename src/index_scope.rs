//! Detects an index whose *scope* is wrong rather than whose contents are
//! (#450, follow-up to #396).
//!
//! `serve`'s existing guardrail refuses an *uninitialized* directory. It says
//! nothing about a directory that is initialized and absurd — a home directory
//! carrying a `.tokensave/` is a perfectly valid project as far as every check
//! upstream of this one is concerned, and every later `serve` inherits it and
//! keeps syncing it. The reported case reached 29.9 GB on disk and a 38 GB
//! footprint once mapped, with the machine paging heavily, and nothing
//! surfaced it.
//!
//! These are warnings, not refusals. Applying #396's cap retroactively would
//! decide for the user which of their existing indexes stop working, and there
//! is no threshold that does not break somebody who is currently fine. So the
//! state is reported, loudly, once per server start, and the user decides —
//! with `suppress_scope_warning` in `.tokensave/config.json` for the user who
//! meant it and does not want to be told again.

use std::path::Path;

/// Size at which an index is worth remarking on. Well above a large monorepo,
/// low enough to catch a runaway before it is paging.
pub const OVERSIZED_INDEX_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// Every scope problem visible from `project_path`, worst first.
///
/// `home` is injected so tests can build both states on disk without mutating
/// process-global environment.
#[must_use]
pub fn scope_warnings(project_path: &Path, home: Option<&Path>) -> Vec<String> {
    let mut out = Vec::new();

    // Checked whatever the working directory is: the whole problem with this
    // state is that nobody is standing in it when it does the damage.
    let home_indexed = home.filter(|home| crate::tokensave::TokenSave::is_initialized(home));
    if let Some(home) = home_indexed {
        out.push(format!(
            "Home directory is initialized as a project: {}/.tokensave/ ({}) \
             — every `serve` started here indexes your whole home tree. \
             Remove it with `rm -rf {}/.tokensave` unless you meant it.",
            home.display(),
            crate::display::format_bytes(index_size_bytes(home)),
            home.display()
        ));
    }

    let is_home = home_indexed.is_some_and(|home| same_directory(home, project_path));
    if !is_home && crate::tokensave::TokenSave::is_initialized(project_path) {
        let size = index_size_bytes(project_path);
        if size >= OVERSIZED_INDEX_BYTES {
            out.push(format!(
                "Index is unusually large: {} ({}) — check `exclude` globs in \
                 .tokensave/config.json; a `serve` maps this whole file.",
                crate::display::format_bytes(size),
                project_path.display()
            ));
        }
    }
    out
}

/// Prints [`scope_warnings`] to stderr, unless the project opted out.
///
/// stderr, never stdout: on a `serve` stdout is the JSON-RPC channel and a
/// stray line there is a protocol error.
pub fn warn_on_serve(project_path: &Path) {
    if crate::config::load_config(project_path).is_ok_and(|c| c.suppress_scope_warning) {
        return;
    }
    for warning in scope_warnings(project_path, crate::agents::home_dir().as_deref()) {
        eprintln!("\x1b[33mwarning:\x1b[0m {warning}");
    }
}

/// On-disk size of a project's index, WAL included — the WAL is part of what a
/// running server maps, and it is where an actively-syncing runaway shows up
/// first.
#[must_use]
pub fn index_size_bytes(project_path: &Path) -> u64 {
    let dir = crate::config::get_tokensave_dir(project_path);
    ["tokensave.db", "tokensave.db-wal"]
        .iter()
        .map(|name| std::fs::metadata(dir.join(name)).map_or(0, |m| m.len()))
        .sum()
}

/// Compare two directories by canonical path, so `~`, a symlink, or a
/// trailing separator does not make the home directory look like a different
/// place than the working directory that is inside it.
#[must_use]
pub fn same_directory(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
