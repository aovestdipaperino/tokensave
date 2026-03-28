//! Tree-sitter grammar provider.
//!
//! All grammars are served from the `tokensave-large-treesitters` bundled
//! crate via a lazily-initialised lookup table.

use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::Language;

/// Cached map of language key -> `Language` built once from the bundled crate.
static LANGUAGES: LazyLock<HashMap<&'static str, Language>> = LazyLock::new(|| {
    tokensave_large_treesitters::all_languages()
        .into_iter()
        .map(|(name, lang_fn)| (name, lang_fn.into()))
        .collect()
});

/// Returns the `tree_sitter::Language` for the given extractor language key.
///
/// # Panics
///
/// Panics if `key` is not recognised.
pub fn language(key: &str) -> Language {
    LANGUAGES
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("ts_provider: unknown language key '{key}'"))
}
