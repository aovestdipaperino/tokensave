//! Tree-sitter grammar provider.
//!
//! All grammars are served from the `tokensave-large-treesitters` bundled
//! crate. Languages not covered by the bundle (Pascal, PowerShell, VB.NET,
//! Objective-C, Batch, COBOL, MSBASIC2, GW-BASIC, QBasic) use their
//! individual crates.

use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::Language;

/// Cached map of language key → `Language` from the bundled crate.
static BUNDLED: LazyLock<HashMap<&'static str, Language>> = LazyLock::new(|| {
    tokensave_large_treesitters::all_languages()
        .into_iter()
        .map(|(name, lang_fn)| (name, lang_fn.into()))
        .collect()
});

/// Returns the `tree_sitter::Language` for the given extractor language key.
///
/// Looks up the bundled crate first; falls back to individual crates for
/// languages not covered by the bundle.
///
/// # Panics
///
/// Panics if `key` is not recognised.
pub fn language(key: &str) -> Language {
    if let Some(lang) = BUNDLED.get(key).cloned() {
        return lang;
    }
    non_bundled_language(key)
}

/// Languages not present in the bundled crate — served from individual deps.
fn non_bundled_language(key: &str) -> Language {
    match key {
        #[cfg(feature = "lang-pascal")]
        "pascal" => tree_sitter_pascal::LANGUAGE.into(),
        #[cfg(feature = "lang-powershell")]
        "powershell" => tree_sitter_powershell::LANGUAGE.into(),
        #[cfg(feature = "lang-vbnet")]
        "vbnet" => tree_sitter_vb_dotnet::LANGUAGE.into(),
        #[cfg(feature = "lang-objc")]
        "objc" => tree_sitter_objc::LANGUAGE.into(),
        #[cfg(feature = "lang-batch")]
        "batch" => tree_sitter_batch::LANGUAGE.into(),
        #[cfg(feature = "lang-cobol")]
        "cobol" => crate::tree_sitter::cobol::LANGUAGE.into(),
        #[cfg(feature = "lang-msbasic2")]
        "msbasic2" => tree_sitter_msbasic2::LANGUAGE.into(),
        #[cfg(feature = "lang-gwbasic")]
        "gwbasic" => tree_sitter_gwbasic::LANGUAGE.into(),
        #[cfg(feature = "lang-qbasic")]
        "qbasic" => tree_sitter_qbasic::LANGUAGE.into(),

        other => panic!("ts_provider: unknown language key '{other}'"),
    }
}
