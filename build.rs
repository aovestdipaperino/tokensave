use std::{fs, path::Path};

fn main() {
    let out_path = Path::new("src/resources/logo.ansi");
    let logo_bytes = include_bytes!("src/resources/logo.png");
    let ansi = logo_art::image_to_ansi(logo_bytes, 90);
    fs::write(out_path, ansi).unwrap();
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
}
