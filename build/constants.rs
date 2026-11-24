//! Named values shared by `build.rs` (#1069).
//!
//! Cargo compiles the build script separately from the crate, so it cannot
//! import crate modules. `build.rs` includes this file with `#[path]`, and
//! `tests/build_constants_test.rs` includes it the same way to check the
//! values against the generated outputs and the source tree.

/// Cargo-provided environment variables that the build script reads.
pub mod cargo_env {
    /// Directory where the build script writes generated files.
    pub const OUT_DIR: &str = "OUT_DIR";
    /// Package root, where the build script looks for commit provenance.
    pub const MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
    /// Path of the compiler used for this build.
    pub const RUSTC: &str = "RUSTC";
    /// Target triple of this build.
    pub const TARGET: &str = "TARGET";
    /// Cargo profile of this build (`debug` or `release`).
    pub const PROFILE: &str = "PROFILE";
}

/// Compile-time environment variables that the build script passes to the
/// crate with `cargo::rustc-env`.
///
/// The crate reads them with `env!`, which only accepts a string literal, so
/// `src/main.rs` repeats these names as literals. That is the one permitted
/// duplicate; `tests/build_constants_test.rs` checks that both sides agree.
pub mod rustc_env {
    /// Abbreviated git commit of the source tree, or [`super::UNKNOWN`].
    pub const BUILD_COMMIT: &str = "TOKENSAVE_BUILD_COMMIT";
    /// `rustc --version` output, or [`super::UNKNOWN`].
    pub const BUILD_RUSTC: &str = "TOKENSAVE_BUILD_RUSTC";
    /// Target triple, or [`super::UNKNOWN`].
    pub const BUILD_TARGET: &str = "TOKENSAVE_BUILD_TARGET";
    /// Cargo profile, or [`super::UNKNOWN`].
    pub const BUILD_PROFILE: &str = "TOKENSAVE_BUILD_PROFILE";
    /// Absolute path of the generated ANSI logo inside `OUT_DIR`.
    pub const LOGO_ANSI_PATH: &str = "TOKENSAVE_BUILD_LOGO_ANSI_PATH";
}

/// Git repository entries that determine the current commit.
pub mod git {
    /// Repository directory at the package root; absent in source archives.
    pub const DIR: &str = ".git";
    /// Reference to the checked-out commit or branch.
    pub const HEAD: &str = "HEAD";
    /// File that stores packed references.
    pub const PACKED_REFS: &str = "packed-refs";
}

/// Provenance file Cargo adds to every package built from a Git checkout
/// (#1062); crates.io archives carry it instead of a `.git` directory.
pub const CARGO_VCS_INFO_FILE_NAME: &str = ".cargo_vcs_info.json";
/// Length of a full SHA-1 commit ID as recorded in [`CARGO_VCS_INFO_FILE_NAME`].
pub const FULL_SHA1_LEN: usize = 40;
/// Suffix added when Cargo recorded uncommitted changes in the package.
pub const DIRTY_SUFFIX: &str = "-dirty";

/// Value reported for build metadata that cannot be determined.
pub const UNKNOWN: &str = "unknown";

/// Number of hexadecimal digits requested for the abbreviated commit.
pub const COMMIT_ABBREV_LEN: usize = 12;
/// Shortest abbreviation git produces; shorter output is rejected.
pub const COMMIT_MIN_LEN: usize = 7;

/// Path components, relative to the package root, of the source logo image.
pub const LOGO_PNG_PATH: &[&str] = &["src", "resources", "logo.png"];
/// File name of the generated ANSI logo inside `OUT_DIR`.
pub const LOGO_ANSI_FILE_NAME: &str = "logo.ansi";
/// Width, in terminal columns, of the generated ANSI logo.
pub const LOGO_ANSI_WIDTH: u32 = 90;

/// Directory, relative to the package root, that holds vendored grammars.
pub const VENDOR_DIR: &str = "vendor";
/// Directory inside each vendored grammar that holds its C sources.
pub const GRAMMAR_SOURCE_DIR: &str = "src";

/// A tree-sitter grammar compiled from vendored C sources.
pub struct VendoredGrammar {
    /// Cargo feature variable set when the grammar's feature is enabled.
    pub feature_env: &'static str,
    /// Directory name under [`VENDOR_DIR`].
    pub directory: &'static str,
    /// Name of the static library that `cc` produces.
    pub library: &'static str,
    /// C sources inside [`GRAMMAR_SOURCE_DIR`].
    pub sources: &'static [&'static str],
}

/// Grammars compiled only when their language feature is enabled.
pub const VENDORED_GRAMMARS: &[VendoredGrammar] = &[
    // Vendored sources avoid tree-sitter-wgsl 0.0.6, which was built against
    // the incompatible tree-sitter 0.20 API.
    VendoredGrammar {
        feature_env: "CARGO_FEATURE_LANG_WGSL",
        directory: "tree-sitter-wgsl",
        library: "tree_sitter_wgsl",
        sources: &["parser.c", "scanner.c"],
    },
    // tree-sitter-actionscript (jcs090218): parser only, no external scanner.
    VendoredGrammar {
        feature_env: "CARGO_FEATURE_LANG_ACTIONSCRIPT",
        directory: "tree-sitter-actionscript",
        library: "tree_sitter_actionscript",
        sources: &["parser.c"],
    },
    // tree-sitter-gdscript (PrestonKnopp): parser plus external scanner.
    VendoredGrammar {
        feature_env: "CARGO_FEATURE_LANG_GDSCRIPT",
        directory: "tree-sitter-gdscript",
        library: "tree_sitter_gdscript",
        sources: &["parser.c", "scanner.c"],
    },
];
