// Rust guideline compliant 2025-10-17
//! Compile/type-check diagnostics, normalised across languages.
//!
//! 5.0 ships the Rust driver (`cargo check --message-format=json`) — the
//! largest single Bash:tokensave gap in the 2026-05-04 telemetry scan
//! (777 invocations). TypeScript (`tsc --noEmit`) and Python (`pyright`)
//! drivers land in follow-up commits.
//!
//! The contract is that every driver returns a `Vec<Diagnostic>` with a
//! consistent shape. The MCP layer enriches each diagnostic with the
//! enclosing graph node, so callers get structured errors mapped to the
//! same node IDs the rest of tokensave's tools speak.

pub mod python;
pub mod rust;
pub mod typescript;

use std::path::Path;

use serde::Serialize;

use crate::errors::Result;

/// One diagnostic emitted by a language's type-checker.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Project-relative file path (e.g. `src/lib.rs`).
    pub file: String,
    /// 1-based start line.
    pub line_start: u32,
    /// 1-based inclusive end line. Equal to `line_start` for single-line spans.
    pub line_end: u32,
    /// Severity. Common values: `"error"`, `"warning"`, `"note"`. Drivers
    /// pass through whatever the compiler reports.
    pub level: String,
    /// Compiler-assigned code (e.g. `"E0308"` for Rust, `"7053"` for TS).
    /// Empty when the compiler didn't attach one.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Driver source — `"rust"`, `"typescript"`, etc. Useful when a project
    /// runs multiple drivers in one pass.
    pub driver: &'static str,
}

/// Scope of the diagnostic run. Drivers may not honor every scope; the
/// `Workspace` variant is the universal fallback.
#[derive(Debug, Clone)]
pub enum Scope {
    /// Whole workspace / project. The default and most expensive scope.
    Workspace,
    /// A single package / cargo crate / TypeScript project root.
    Package { name: String },
    /// A single file. Most useful for editor-style on-save checks.
    File { path: String },
}

/// Per-language driver contract. Implementations live in submodules
/// (`rust`, `typescript`, `python`, …).
pub trait Driver {
    /// Driver identifier (`"rust"`, `"typescript"`, `"python"`).
    fn name(&self) -> &'static str;

    /// True when `project_root` looks like the kind of project this driver
    /// handles. Cheap probe — typically existence of a manifest file.
    fn detect(&self, project_root: &Path) -> bool;

    /// Run the diagnostic pass over `scope`. Implementations are async
    /// because they shell out to the compiler.
    fn run<'a>(
        &'a self,
        project_root: &'a Path,
        scope: &'a Scope,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Diagnostic>>> + Send + 'a>>;
}

/// Run every detected driver against `project_root` and return the merged
/// diagnostic list. Drivers are run sequentially; any driver-level error
/// is propagated immediately. Empty when no driver detects the project.
pub async fn run_all(project_root: &Path, scope: &Scope) -> Result<Vec<Diagnostic>> {
    let drivers: Vec<Box<dyn Driver + Send + Sync>> = vec![
        Box::new(rust::CargoDriver),
        Box::new(typescript::TscDriver),
        Box::new(python::PyrightDriver),
    ];

    let mut all = Vec::new();
    for driver in drivers {
        if !driver.detect(project_root) {
            continue;
        }
        let mut diags = driver.run(project_root, scope).await?;
        all.append(&mut diags);
    }
    Ok(all)
}
