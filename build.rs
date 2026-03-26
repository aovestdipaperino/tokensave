use std::{fs, path::Path};

fn main() {
    let out_path = Path::new("src/resources/logo.ansi");
    let logo_bytes = include_bytes!("src/resources/logo.png");
    let ansi = logo_art::image_to_ansi(logo_bytes, 90);
    fs::write(out_path, ansi).unwrap();
    println!("cargo::rerun-if-changed=src/resources/logo.png");

    // Compile vendored tree-sitter-protobuf grammar (no compatible crate for tree-sitter 0.26)
    let proto_dir = Path::new("vendor/tree-sitter-protobuf/src");
    cc::Build::new()
        .include(proto_dir)
        .file(proto_dir.join("parser.c"))
        .warnings(false)
        .compile("tree_sitter_protobuf");
    println!("cargo::rerun-if-changed=vendor/tree-sitter-protobuf/src/parser.c");
}
