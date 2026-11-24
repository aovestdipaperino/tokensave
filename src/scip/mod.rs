//! Experimental SCIP interoperability prototype (#1100).
//!
//! Reads a [SCIP](https://github.com/sourcegraph/scip) index produced by an
//! external, compiler-backed indexer and compares it with the tokensave graph
//! without writing to it. See `docs/scip/README.md` for the format mapping,
//! limits, and evaluation.

pub mod compare;
pub mod symbol;
pub mod wire;

pub use compare::{compare, Options};
