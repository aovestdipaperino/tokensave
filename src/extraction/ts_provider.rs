//! Tree-sitter grammar provider.
//!
//! Centralises how extractors obtain their `tree_sitter::Language`.
//! Controlled by compile-time feature flags:
//!
//! - **`ts-ffi`** — individual `tree-sitter-*` crates (classic FFI bindings).
//! - **`ts-rust`** — bundled `tokensave-*-treesitters` crates.
//! - **`ts-both`** — both sources; the bundled crate is primary, FFI is used
//!   for comparison (see [`with_ffi_override`]).

use std::cell::Cell;
use tree_sitter::Language;
#[cfg(feature = "ts-both")]
use quantum_pulse::ProfileCollector;

// ---------------------------------------------------------------------------
// Thread-local override for "both" mode
// ---------------------------------------------------------------------------

thread_local! {
    /// When `true`, [`language`] returns the FFI variant instead of bundled.
    /// Only meaningful under `ts-both`.
    static FORCE_FFI: Cell<bool> = const { Cell::new(false) };
}

/// Run `f` with the language provider forced to the FFI source.
///
/// Used in `ts-both` mode to re-extract a file with the alternate grammar
/// so the results can be compared.
#[cfg(feature = "ts-both")]
pub fn with_ffi_override<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    FORCE_FFI.set(true);
    let result = f();
    FORCE_FFI.set(false);
    result
}

// ---------------------------------------------------------------------------
// Primary entry point
// ---------------------------------------------------------------------------

/// Returns the `tree_sitter::Language` for the given extractor language key.
///
/// The key is a lowercase identifier matching the names used in each
/// extractor (see [`ffi_language`] for the full list).
///
/// Under `ts-rust` / `ts-both` the bundled crate is preferred for
/// languages it covers; languages outside the bundle always come from FFI.
///
/// # Panics
///
/// Panics if `key` is not recognised.
pub fn language(key: &str) -> Language {
    #[cfg(feature = "ts-both")]
    {
        if FORCE_FFI.get() {
            return ffi_language(key);
        }
    }

    #[cfg(feature = "ts-rust")]
    {
        if let Some(lang) = bundled_language(key) {
            return lang;
        }
    }

    ffi_language(key)
}

// ---------------------------------------------------------------------------
// FFI source (individual tree-sitter-* crates)
// ---------------------------------------------------------------------------

/// Returns the language from the individual FFI crate.
fn ffi_language(key: &str) -> Language {
    match key {
        // Lite — always available
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "scala" => tree_sitter_scala::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "c" => tree_sitter_c::LANGUAGE.into(),
        "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        "kotlin" => tree_sitter_kotlin_sg::LANGUAGE.into(),
        "c_sharp" => tree_sitter_c_sharp::LANGUAGE.into(),
        "swift" => tree_sitter_swift::LANGUAGE.into(),

        // Medium
        #[cfg(feature = "lang-dart")]
        "dart" => tree_sitter_dart_orchard::LANGUAGE.into(),
        #[cfg(feature = "lang-pascal")]
        "pascal" => tree_sitter_pascal::LANGUAGE.into(),
        #[cfg(feature = "lang-php")]
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        #[cfg(feature = "lang-ruby")]
        "ruby" => tree_sitter_ruby::LANGUAGE.into(),
        #[cfg(feature = "lang-bash")]
        "bash" => tree_sitter_bash::LANGUAGE.into(),
        #[cfg(feature = "lang-protobuf")]
        "protobuf" => crate::tree_sitter::protobuf::LANGUAGE.into(),
        #[cfg(feature = "lang-powershell")]
        "powershell" => tree_sitter_powershell::LANGUAGE.into(),
        #[cfg(feature = "lang-nix")]
        "nix" => tree_sitter_nix::LANGUAGE.into(),
        #[cfg(feature = "lang-vbnet")]
        "vbnet" => tree_sitter_vb_dotnet::LANGUAGE.into(),

        // Full
        #[cfg(feature = "lang-lua")]
        "lua" => tree_sitter_lua::LANGUAGE.into(),
        #[cfg(feature = "lang-zig")]
        "zig" => tree_sitter_zig::LANGUAGE.into(),
        #[cfg(feature = "lang-objc")]
        "objc" => tree_sitter_objc::LANGUAGE.into(),
        #[cfg(feature = "lang-perl")]
        "perl" => tree_sitter_perl::LANGUAGE.into(),
        #[cfg(feature = "lang-batch")]
        "batch" => tree_sitter_batch::LANGUAGE.into(),
        #[cfg(feature = "lang-fortran")]
        "fortran" => tree_sitter_fortran::LANGUAGE.into(),
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

// ---------------------------------------------------------------------------
// Bundled source (tokensave-*-treesitters crates)
// ---------------------------------------------------------------------------

/// Returns the language from the bundled crate, or `None` if the language
/// is not covered by the bundle (those always fall back to FFI).
#[cfg(feature = "ts-rust")]
fn bundled_language(key: &str) -> Option<Language> {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    static BUNDLED: LazyLock<HashMap<&'static str, Language>> = LazyLock::new(|| {
        tokensave_large_treesitters::all_languages()
            .into_iter()
            .map(|(name, lang_fn)| (name, lang_fn.into()))
            .collect()
    });

    BUNDLED.get(key).cloned()
}

// ---------------------------------------------------------------------------
// Comparison logic for "both" mode
// ---------------------------------------------------------------------------

/// Compare two nodes ignoring `updated_at` (which differs between runs).
#[cfg(feature = "ts-both")]
fn nodes_eq(a: &crate::types::Node, b: &crate::types::Node) -> bool {
    a.id == b.id
        && a.kind == b.kind
        && a.name == b.name
        && a.qualified_name == b.qualified_name
        && a.file_path == b.file_path
        && a.start_line == b.start_line
        && a.end_line == b.end_line
        && a.start_column == b.start_column
        && a.end_column == b.end_column
        && a.signature == b.signature
        && a.docstring == b.docstring
        && a.visibility == b.visibility
        && a.is_async == b.is_async
        && a.branches == b.branches
        && a.loops == b.loops
        && a.returns == b.returns
        && a.max_nesting == b.max_nesting
        && a.unsafe_blocks == b.unsafe_blocks
        && a.unchecked_calls == b.unchecked_calls
        && a.assertions == b.assertions
}

/// Profile an extraction pass and record its duration under the given key.
#[cfg(feature = "ts-both")]
pub fn profile_extraction<F>(key: &str, f: F) -> crate::types::ExtractionResult
where
    F: FnOnce() -> crate::types::ExtractionResult,
{
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed().as_micros() as u64;
    ProfileCollector::record(key, elapsed);
    result
}

/// Print a performance comparison table: bundled vs FFI extraction times.
#[cfg(feature = "ts-both")]
pub fn print_perf_summary() {
    let bundled = ProfileCollector::get_stats("extract:bundled");
    let ffi = ProfileCollector::get_stats("extract:ffi");

    let (b_count, b_total) = match &bundled {
        Some(s) => (s.count, s.total),
        None => return, // no data
    };
    let (f_count, f_total) = match &ffi {
        Some(s) => (s.count, s.total),
        None => return,
    };

    if b_count == 0 {
        return;
    }

    let b_ms = b_total.as_secs_f64() * 1000.0;
    let f_ms = f_total.as_secs_f64() * 1000.0;
    let b_mean = b_ms / b_count as f64;
    let f_mean = f_ms / f_count as f64;
    let diff_pct = if f_ms > 0.0 {
        ((b_ms - f_ms) / f_ms) * 100.0
    } else {
        0.0
    };
    let faster = if b_ms < f_ms { "bundled" } else { "ffi" };

    eprintln!();
    eprintln!("╭─ grammar performance ─────────────────────────────────────╮");
    eprintln!("│  {:>10}  {:>10}  {:>10}  {:>10}               │", "source", "files", "total", "mean");
    eprintln!("│  {:>10}  {:>10}  {:>9.1}ms  {:>8.3}ms               │", "bundled", b_count, b_ms, b_mean);
    eprintln!("│  {:>10}  {:>10}  {:>9.1}ms  {:>8.3}ms               │", "ffi", f_count, f_ms, f_mean);
    eprintln!("│                                                           │");
    eprintln!(
        "│  {} is {:.1}% faster                                    │",
        faster,
        diff_pct.abs()
    );
    eprintln!("╰───────────────────────────────────────────────────────────╯");
    eprintln!();
}

/// Compares two extraction results and prints a bug report if they differ.
///
/// Called during index/sync when `ts-both` is enabled. The `primary` result
/// (from the bundled crate) is always the one stored in the DB; the `ffi`
/// result is only used for comparison.
///
/// Ignores `updated_at` on nodes and `duration_ms` on results since those
/// are timestamps that naturally differ between two extraction passes.
#[cfg(feature = "ts-both")]
pub fn compare_extractions(
    file_path: &str,
    language: &str,
    primary: &crate::types::ExtractionResult,
    ffi: &crate::types::ExtractionResult,
) {
    let node_match = primary.nodes.len() == ffi.nodes.len()
        && primary.nodes.iter().zip(ffi.nodes.iter()).all(|(a, b)| nodes_eq(a, b));
    let edge_match = primary.edges.len() == ffi.edges.len()
        && primary.edges.iter().zip(ffi.edges.iter()).all(|(a, b)| a == b);
    let refs_match = primary.unresolved_refs.len() == ffi.unresolved_refs.len()
        && primary
            .unresolved_refs
            .iter()
            .zip(ffi.unresolved_refs.iter())
            .all(|(a, b)| a == b);

    if node_match && edge_match && refs_match {
        return;
    }

    print_bug_report(file_path, language, primary, ffi, node_match, edge_match, refs_match);
}

#[cfg(feature = "ts-both")]
fn print_bug_report(
    file_path: &str,
    language: &str,
    primary: &crate::types::ExtractionResult,
    ffi: &crate::types::ExtractionResult,
    node_match: bool,
    edge_match: bool,
    refs_match: bool,
) {
    let version = env!("CARGO_PKG_VERSION");

    eprintln!();
    eprintln!("\x1b[33m╭─ tree-sitter grammar discrepancy ─────────────────────────╮\x1b[0m");
    eprintln!("\x1b[33m│\x1b[0m File:     {file_path}");
    eprintln!("\x1b[33m│\x1b[0m Language: {language}");
    eprintln!("\x1b[33m│\x1b[0m Version:  {version}");
    eprintln!("\x1b[33m│\x1b[0m");

    if !node_match {
        eprintln!(
            "\x1b[33m│\x1b[0m  Nodes: bundled={}, ffi={}",
            primary.nodes.len(),
            ffi.nodes.len()
        );
    }
    if !edge_match {
        eprintln!(
            "\x1b[33m│\x1b[0m  Edges: bundled={}, ffi={}",
            primary.edges.len(),
            ffi.edges.len()
        );
    }
    if !refs_match {
        eprintln!(
            "\x1b[33m│\x1b[0m  Unresolved refs: bundled={}, ffi={}",
            primary.unresolved_refs.len(),
            ffi.unresolved_refs.len()
        );
    }

    eprintln!("\x1b[33m│\x1b[0m");
    eprintln!("\x1b[33m│\x1b[0m Please report this — copy everything above and open:");
    eprintln!(
        "\x1b[33m│\x1b[0m \x1b[4mhttps://github.com/aovestdipaperino/tokensave/issues/new?title=ts-both+discrepancy:+{language}+({file_path})&labels=ts-both\x1b[0m"
    );
    eprintln!("\x1b[33m╰───────────────────────────────────────────────────────────╯\x1b[0m");
    eprintln!();
}
