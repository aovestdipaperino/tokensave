//! Project-index storage.
//!
//! Every read and write of a project index goes through [`Database`]; code
//! outside this module never issues storage queries (#904). That API is the
//! storage contract: a backend implements the same `Database` methods and is
//! selected at build time, so callers and the shared test suite stay
//! unchanged across backends. File-level operations that depend on the
//! engine's on-disk format, such as copying an index for a new branch
//! ([`snapshot`]), live here as well.
//!
//! libSQL is the default backend; the experimental `grafeo` feature selects
//! a Grafeo backend instead.

// SQLite modules the global database uses in every build.
pub mod compatibility;
pub mod debug_sql;
pub mod migrations;
pub mod model;

// Backend-neutral helpers both project-index backends share.
pub(crate) mod fts_query;
pub(crate) mod language;
mod rails_routes;

// libSQL project-index backend (the default).
#[cfg(not(feature = "grafeo"))]
mod connection;
#[cfg(not(feature = "grafeo"))]
pub mod probe;
#[cfg(not(feature = "grafeo"))]
mod profiled;
#[cfg(not(feature = "grafeo"))]
mod queries;
#[cfg(not(feature = "grafeo"))]
pub mod snapshot;

#[cfg(not(feature = "grafeo"))]
pub use connection::Database;
#[cfg(not(feature = "grafeo"))]
pub(crate) use profiled::RowSource;

/// Name of the storage engine this build uses. Profiling records are kept
/// per engine, so measurements of different backends never merge.
#[cfg(not(feature = "grafeo"))]
pub const ENGINE: &str = "libsql";

// Experimental Grafeo project-index backend (#904).
#[cfg(feature = "grafeo")]
mod grafeo;
#[cfg(feature = "grafeo")]
pub use self::grafeo::{probe, snapshot, Database};

/// Name of the storage engine this build uses. Profiling records are kept
/// per engine, so measurements of different backends never merge.
#[cfg(feature = "grafeo")]
pub const ENGINE: &str = "grafeo";

pub use model::{AuditEdgeRow, EdgeEndpoint, ImportEdgeRow, NodeFilter, StoredFingerprint};
