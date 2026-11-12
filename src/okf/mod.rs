//! Open Knowledge Format (OKF) interchange (#858).
//!
//! OKF is a directory of markdown files with YAML frontmatter
//! (<https://github.com/GoogleCloudPlatform/open-knowledge-format>). This
//! module implements OKF v0.2:
//!
//! - [`bundle`] reads a bundle with bounded resources;
//! - [`validate`] checks it against the conformance rules (§11) and reports
//!   actionable diagnostics;
//! - [`export`] writes the indexed code graph as a bundle.
//!
//! The compliance matrix is in `docs/guide/OKF.md`.

pub mod bundle;
pub mod document;
pub mod export;
pub mod time;
pub mod validate;
pub mod yaml;

/// The OKF specification version this module implements.
pub const SPEC_VERSION: &str = "0.2";

/// Commit of the specification repository the implementation was checked
/// against, including its sample bundles.
pub const SPEC_COMMIT: &str = "ad30107c31c06aec8a7d5636e0d1058118604e6f";

/// Reads and validates the bundle at `root`.
pub fn validate_dir(root: &std::path::Path) -> std::io::Result<validate::Report> {
    let bundle = bundle::read_default(root)?;
    Ok(validate::validate(&bundle, time::now()))
}
