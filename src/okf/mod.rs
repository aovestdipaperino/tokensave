//! Open Knowledge Format (OKF) interchange (#858, #1060).
//!
//! OKF is a directory of markdown files with YAML frontmatter
//! (<https://github.com/open-knowledge-format/open-knowledge-format>). This
//! module implements OKF v0.3 and reads bundles of earlier versions with the
//! fallbacks of §12.1:
//!
//! - [`bundle`] reads a bundle with bounded resources and resolves links and
//!   path fields;
//! - [`consumer`] holds the version, metadata, lifecycle and attestation
//!   rules shared by the validator and the search;
//! - [`trust`] computes trust tiers and freshness;
//! - [`validate`] checks a bundle against the conformance rules (§11) and
//!   reports actionable diagnostics;
//! - [`export`] writes the indexed code graph as a bundle;
//! - [`store`] imports bundles into the project state directory;
//! - [`knowledge`] searches imported bundles for the MCP server.
//!
//! The compliance matrix is in `docs/guide/OKF.md`.

pub mod bundle;
pub mod consumer;
pub mod document;
pub mod export;
pub mod knowledge;
pub mod store;
pub mod time;
pub mod trust;
pub mod validate;
pub mod yaml;

/// The OKF specification version this module implements.
pub const SPEC_VERSION: &str = "0.3";

/// Commit of the specification repository the implementation was checked
/// against, including its sample bundles.
pub const SPEC_COMMIT: &str = "527e81aff55992be21507e97004286789126f88c";

/// Reads and validates the bundle at `root`.
pub fn validate_dir(root: &std::path::Path) -> std::io::Result<validate::Report> {
    let bundle = bundle::read_default(root)?;
    Ok(validate::validate(&bundle, time::now()))
}
