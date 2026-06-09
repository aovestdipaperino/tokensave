//! Shared types used by every ecosystem parser.
//!
//! Each ecosystem (Rust / Node / Python / Go / Java / .NET / PHP / Ruby)
//! produces a `Workspace` describing its members and their declared
//! dependencies. The top-level handler renders one shape across ecosystems so
//! the MCP response stays uniform regardless of language.

use std::path::PathBuf;

use serde_json::{json, Value};

/// Conceptual kind of a declared dependency. Each ecosystem maps its own
/// section names to one of these — the labels are not free-form strings to
/// keep `kind` filtering predictable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
    Peer,
    Optional,
    /// Free-form fallback for ecosystem-specific kinds (e.g. Composer
    /// `replace`, Cargo `target-cfg`-scoped reuse).
    Other(&'static str),
}

impl DepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DepKind::Normal => "normal",
            DepKind::Dev => "dev",
            DepKind::Build => "build",
            DepKind::Peer => "peer",
            DepKind::Optional => "optional",
            DepKind::Other(s) => s,
        }
    }

    /// Does this kind match the `--kind` filter passed by the user?
    /// `"all"` matches everything; specific filters match only the named
    /// kind.
    pub fn passes(self, filter: &str) -> bool {
        if filter == "all" {
            return true;
        }
        self.as_str() == filter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub version: Option<String>,
    pub features: Vec<String>,
    pub optional: bool,
    /// `path = ".."` / `"file:../local"` / etc. — workspace-local deps.
    pub local_path: Option<String>,
    pub kind: DepKind,
}

#[derive(Debug, Clone)]
pub struct Member {
    /// Path relative to the workspace root. Use `"."` for a single-package
    /// project.
    pub path: String,
    /// Display name of this member/package.
    pub name: String,
    pub deps: Vec<Dep>,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub ecosystem: &'static str,
    pub root: PathBuf,
    pub members: Vec<Member>,
    /// Free-form ecosystem-specific notes (e.g. Cargo `[patch.crates-io]`
    /// entries, Go `replace` directives, npm `overrides`). Rendered verbatim
    /// in the MCP response.
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone)]
pub struct Patch {
    pub source: String,
    pub name: String,
    pub replacement: String,
}

pub fn dep_to_json(d: &Dep) -> Value {
    json!({
        "name": d.name,
        "kind": d.kind.as_str(),
        "version": d.version,
        "features": d.features,
        "optional": d.optional,
        "path": d.local_path,
    })
}

pub fn patch_to_json(p: &Patch) -> Value {
    json!({
        "source": p.source,
        "name": p.name,
        "replacement": p.replacement,
    })
}
