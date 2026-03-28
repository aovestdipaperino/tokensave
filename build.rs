use std::{fs, path::Path};

fn main() {
    let out_path = Path::new("src/resources/logo.ansi");
    let logo_bytes = include_bytes!("src/resources/logo.png");
    let ansi = logo_art::image_to_ansi(logo_bytes, 90);
    fs::write(out_path, ansi).unwrap();
    println!("cargo::rerun-if-changed=src/resources/logo.png");

    // Compile vendored tree-sitter-cobol grammar (no compatible crate for tree-sitter 0.26)
    if std::env::var("CARGO_FEATURE_LANG_COBOL").is_ok() {
        let cobol_dir = Path::new("vendor/tree-sitter-cobol/src");
        cc::Build::new()
            .include(cobol_dir)
            .file(cobol_dir.join("parser.c"))
            .file(cobol_dir.join("scanner.c"))
            .warnings(false)
            .compile("tree_sitter_cobol");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-cobol/src/parser.c");
        println!("cargo::rerun-if-changed=vendor/tree-sitter-cobol/src/scanner.c");
    }
}
