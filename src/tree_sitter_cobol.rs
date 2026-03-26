//! Vendored tree-sitter-cobol language binding.
//!
//! The `tree-sitter-cobol` crate on crates.io is a placeholder with no lib
//! target. We vendor the grammar's C source from
//! <https://github.com/yutaro-sakamoto/tree-sitter-cobol> (compiled via
//! build.rs) and expose it through tree-sitter-language's `LanguageFn`,
//! which is the modern tree-sitter 0.24+ API.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_COBOL() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for COBOL grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_COBOL) };
