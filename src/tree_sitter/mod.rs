//! Vendored tree-sitter language bindings.
//!
//! Grammars whose crates are missing or have incompatible tree-sitter
//! versions are compiled from C source via `build.rs` and exposed here.

#[cfg(feature = "lang-cobol")]
pub mod cobol;
