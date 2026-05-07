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

#[cfg(test)]
mod tests {
    /// Every key that an extractor passes to `language()` must be present in the
    /// grammar table. Add new entries here whenever a new extractor is added.
    #[test]
    fn all_extractor_keys_are_registered() {
        #[rustfmt::skip]
        let keys = [
            "bash", "batch", "c", "c_sharp", "clojure", "cobol", "cpp", "dart",
            "dockerfile", "elixir", "erlang", "fortran", "fsharp", "glsl", "go",
            "gwbasic", "haskell", "java", "javascript", "julia", "kotlin", "lean", "lua",
            "msbasic2", "nix", "objc", "ocaml", "pascal", "perl", "php", "powershell",
            "protobuf", "python", "qbasic", "quint", "r", "ruby", "rust", "scala", "sql",
            "swift", "toml", "tsx", "typescript", "vbnet", "zig",
        ];
        let missing: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|k| super::LANGUAGES.get(k).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "grammar keys missing from LANGUAGES: {missing:?}"
        );
    }
}
