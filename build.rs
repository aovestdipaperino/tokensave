use std::{fs, path::Path, process::Command};

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty() && !value.contains(['\n', '\r'])).then_some(value)
}

fn build_metadata() {
    // Do not pick up an unrelated parent repository for unpacked source archives.
    let commit = if Path::new(".git").exists() {
        let current_ref = command_output("git", &["symbolic-ref", "-q", "HEAD"]);
        for name in [Some("HEAD"), Some("packed-refs"), current_ref.as_deref()]
            .into_iter()
            .flatten()
        {
            if let Some(path) = command_output("git", &["rev-parse", "--git-path", name]) {
                println!("cargo::rerun-if-changed={path}");
            }
        }
        command_output("git", &["rev-parse", "--short=12", "HEAD"])
            .filter(|sha| sha.len() >= 7 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
    } else {
        None
    };
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
