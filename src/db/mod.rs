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
//! libSQL is the only backend so far.

pub mod compatibility;
mod connection;
pub mod debug_sql;
pub mod migrations;
pub mod probe;
mod profiled;
mod queries;
pub mod snapshot;

pub use connection::Database;

/// Name of the storage engine this build uses. Profiling records are kept
/// per engine, so measurements of different backends never merge.
pub const ENGINE: &str = "libsql";
pub(crate) use profiled::RowSource;
pub use queries::{AuditEdgeRow, EdgeEndpoint, ImportEdgeRow, NodeFilter};
