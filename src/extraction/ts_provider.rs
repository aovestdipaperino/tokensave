//! Tree-sitter grammar provider.
//!
//! All grammars are served from the `tokensave-large-treesitters` bundled
//! crate via a lazily-initialised lookup table.

use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::Language;

// tree-sitter-wgsl 0.0.6 was built against tree-sitter 0.20, whose Language
// type is not assignment-compatible with 0.26. Re-declare the raw C symbol so
// we can construct a LanguageFn with the correct pointer type directly.
#[cfg(feature = "lang-wgsl")]
mod wgsl_grammar {
    use tree_sitter_language::LanguageFn;

    // Grammar compiled from vendor/tree-sitter-wgsl/src/ via build.rs.
    unsafe extern "C" {
        fn tree_sitter_wgsl() -> *const ();
    }
    pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_wgsl) };
}

// Vendored ActionScript grammar (tree-sitter-actionscript, jcs090218),
// compiled from vendor/tree-sitter-actionscript/src/ via build.rs.
#[cfg(feature = "lang-actionscript")]
mod actionscript_grammar {
    use tree_sitter_language::LanguageFn;

    unsafe extern "C" {
        fn tree_sitter_actionscript() -> *const ();
    }
    pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_actionscript) };
}

// Vendored GDScript grammar (tree-sitter-gdscript, PrestonKnopp),
// compiled from vendor/tree-sitter-gdscript/src/ via build.rs (parser.c +
// external scanner.c).
#[cfg(feature = "lang-gdscript")]
mod gdscript_grammar {
    use tree_sitter_language::LanguageFn;

    unsafe extern "C" {
        fn tree_sitter_gdscript() -> *const ();
    }
    pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_gdscript) };
}

/// Cached map of language key -> `Language` built once from the bundled crate.
static LANGUAGES: LazyLock<HashMap<&'static str, Language>> = LazyLock::new(|| {
    #[allow(unused_mut)]
    let mut map: HashMap<&'static str, Language> = tokensave_large_treesitters::all_languages()
        .into_iter()
        .map(|(name, lang_fn)| (name, lang_fn.into()))
        .collect();

    #[cfg(feature = "lang-wgsl")]
    map.insert("wgsl", wgsl_grammar::LANGUAGE.into());

    #[cfg(feature = "lang-actionscript")]
    map.insert("actionscript", actionscript_grammar::LANGUAGE.into());

    #[cfg(feature = "lang-gdscript")]
    map.insert("gdscript", gdscript_grammar::LANGUAGE.into());

    // HLSL uses the newer LanguageFn API.
    #[cfg(feature = "lang-hlsl")]
    map.insert("hlsl", tree_sitter_hlsl::LANGUAGE_HLSL.into());
    #[cfg(feature = "lang-systemverilog")]
    map.insert("systemverilog", tree_sitter_systemverilog::LANGUAGE.into());
    #[cfg(feature = "lang-html")]
    map.insert("html", tree_sitter_html::LANGUAGE.into());
    #[cfg(feature = "lang-css")]
    map.insert("css", tree_sitter_css::LANGUAGE.into());
    #[cfg(feature = "lang-vhdl")]
    map.insert("vhdl", tree_sitter_vhdl::LANGUAGE.into());

    map
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

/// Returns the grammar for a source file, chosen by its extension, or `None`
/// when no bundled grammar covers it.
///
/// Used by tools that re-parse a file outside extraction — `tokensave_rename`
/// locates identifier tokens with it and checks that an edited file still
/// parses. Unlike [`language`], an unknown extension is not a bug, so this
/// never panics.
#[must_use]
pub fn language_for_path(path: &str) -> Option<Language> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    let key = match ext.as_str() {
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "scala" | "sc" => "scala",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "inl" | "ipp" | "tcc" => "cpp",
        "cs" => "c_sharp",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" | "rake" => "ruby",
        "php" => "php",
        "dart" => "dart",
        "lua" => "lua",
        "zig" => "zig",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "fs" | "fsi" | "fsx" => "fsharp",
        "jl" => "julia",
        "r" => "r",
        "sh" | "bash" => "bash",
        "pl" | "pm" => "perl",
        "ps1" | "psm1" => "powershell",
        "nix" => "nix",
        "proto" => "protobuf",
        "m" => "objc",
        "clj" | "cljs" | "cljc" => "clojure",
        "gd" => "gdscript",
        "as" => "actionscript",
        _ => return None,
    };
    LANGUAGES.get(key).cloned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn language_for_path_maps_common_extensions() {
        assert!(super::language_for_path("src/lib.rs").is_some());
        assert!(super::language_for_path("app/main.py").is_some());
        assert!(super::language_for_path("web/App.TSX").is_some());
        assert!(super::language_for_path("README.md").is_none());
        assert!(super::language_for_path("Makefile").is_none());
    }

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
        // Keys provided by optional direct deps — checked separately so the test
        // is skipped when the feature is not enabled.
        #[cfg(feature = "lang-wgsl")]
        assert!(
            super::LANGUAGES.get("wgsl").is_some(),
            "wgsl grammar missing"
        );
        #[cfg(feature = "lang-hlsl")]
        assert!(
            super::LANGUAGES.get("hlsl").is_some(),
            "hlsl grammar missing"
        );
        #[cfg(feature = "lang-actionscript")]
        assert!(
            super::LANGUAGES.get("actionscript").is_some(),
            "actionscript grammar missing"
        );
        #[cfg(feature = "lang-gdscript")]
        assert!(
            super::LANGUAGES.get("gdscript").is_some(),
            "gdscript grammar missing"
        );
        #[cfg(feature = "lang-html")]
        assert!(
            super::LANGUAGES.get("html").is_some(),
            "html grammar missing"
        );
        #[cfg(feature = "lang-css")]
        assert!(super::LANGUAGES.get("css").is_some(), "css grammar missing");
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
