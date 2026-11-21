use std::{fs, path::Path, process::Command};

#[path = "build/provenance.rs"]
mod provenance;

fn build_metadata() {
    // Git checkouts use `git rev-parse`; packaged crates (crates.io) use the
    // `.cargo_vcs_info.json` Cargo writes at package time (#1062). Only the
    // package root is inspected, so an unrelated parent repository is ignored.
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| ".".into());
    let provenance = provenance::resolve(Path::new(&manifest_dir));
    for path in &provenance.rerun_if_changed {
        println!("cargo::rerun-if-changed={path}");
    }
    let commit = provenance.commit;
    let rustc = std::env::var_os("RUSTC")
        .and_then(|path| Command::new(path).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && !value.contains(['\n', '\r']));
    println!(
        "cargo::rustc-env=TOKENSAVE_BUILD_COMMIT={}",
        commit.as_deref().unwrap_or("unknown")
    );
    println!(
        "cargo::rustc-env=TOKENSAVE_BUILD_RUSTC={}",
        rustc.as_deref().unwrap_or("unknown")
    );
    println!(
        "cargo::rustc-env=TOKENSAVE_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
    println!(
        "cargo::rustc-env=TOKENSAVE_BUILD_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".into())
    );
}

fn main() {
    build_metadata();
    let out_dir = std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo");
    let out_path = Path::new(&out_dir).join("logo.ansi");
    let logo_bytes = include_bytes!("src/resources/logo.png");
    let ansi = logo_art::image_to_ansi(logo_bytes, 90);
    fs::write(&out_path, ansi).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
    println!("cargo::rerun-if-changed=src/resources/logo.png");

    // Vendored WGSL grammar — compiled only when lang-wgsl is enabled.
    // Using vendored sources avoids pulling in tree-sitter-wgsl 0.0.6 which was
    // built against the incompatible tree-sitter 0.20 API.
    if std::env::var("CARGO_FEATURE_LANG_WGSL").is_ok() {
        let wgsl_dir = Path::new("vendor/tree-sitter-wgsl/src");
        cc::Build::new()
            .include(wgsl_dir)
            .file(wgsl_dir.join("parser.c"))
            .file(wgsl_dir.join("scanner.c"))
            .warnings(false)
            .compile("tree_sitter_wgsl");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-wgsl/src/parser.c");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-wgsl/src/scanner.c");
    }

    // Vendored ActionScript grammar (tree-sitter-actionscript, jcs090218).
    // parser.c only — the grammar has no external scanner.
    if std::env::var("CARGO_FEATURE_LANG_ACTIONSCRIPT").is_ok() {
        let as_dir = Path::new("vendor/tree-sitter-actionscript/src");
        cc::Build::new()
            .include(as_dir)
            .file(as_dir.join("parser.c"))
            .warnings(false)
            .compile("tree_sitter_actionscript");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-actionscript/src/parser.c");
    }

    // Vendored GDScript grammar (tree-sitter-gdscript, PrestonKnopp).
    // parser.c + scanner.c — the grammar has an external scanner.
    if std::env::var("CARGO_FEATURE_LANG_GDSCRIPT").is_ok() {
        let gdscript_dir = Path::new("vendor/tree-sitter-gdscript/src");
        cc::Build::new()
            .include(gdscript_dir)
            .file(gdscript_dir.join("parser.c"))
            .file(gdscript_dir.join("scanner.c"))
            .warnings(false)
            .compile("tree_sitter_gdscript");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-gdscript/src/parser.c");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-gdscript/src/scanner.c");
    }
}
