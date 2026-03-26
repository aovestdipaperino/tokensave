//! Vendored tree-sitter-protobuf language binding.
//!
//! The `devgen-tree-sitter-protobuf` crate depends on tree-sitter 0.21 which
//! conflicts with the project's tree-sitter 0.26. We vendor the grammar's C
//! source (compiled via build.rs) and expose it through tree-sitter-language's
//! `LanguageFn`, which is the modern tree-sitter 0.24+ API.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_protobuf() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for protobuf grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_protobuf) };
