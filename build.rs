use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[path = "build/constants.rs"]
mod constants;

use constants::{cargo_env, git, rustc_env};

#[path = "build/provenance.rs"]
mod provenance;

fn emit_rustc_env(key: &str, value: Option<&str>) {
    println!(
        "cargo::rustc-env={key}={}",
        value.unwrap_or(constants::UNKNOWN)
    );
}

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
    // Do not pick up an unrelated parent repository for unpacked source archives.
    let commit = if Path::new(git::DIR).exists() {
        let current_ref = command_output("git", &["symbolic-ref", "-q", git::HEAD]);
        for name in [
            Some(git::HEAD),
            Some(git::PACKED_REFS),
            current_ref.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(path) = command_output("git", &["rev-parse", "--git-path", name]) {
                println!("cargo::rerun-if-changed={path}");
            }
        }
        let abbrev = format!("--short={}", constants::COMMIT_ABBREV_LEN);
        command_output("git", &["rev-parse", &abbrev, git::HEAD]).filter(|sha| {
            sha.len() >= constants::COMMIT_MIN_LEN && sha.bytes().all(|b| b.is_ascii_hexdigit())
        })
    } else {
        None
    };
    let rustc = std::env::var_os(cargo_env::RUSTC)
        .and_then(|path| Command::new(path).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && !value.contains(['\n', '\r']));
    emit_rustc_env(rustc_env::BUILD_COMMIT, commit.as_deref());
    emit_rustc_env(rustc_env::BUILD_RUSTC, rustc.as_deref());
    emit_rustc_env(
        rustc_env::BUILD_TARGET,
        std::env::var(cargo_env::TARGET).ok().as_deref(),
    );
    emit_rustc_env(
        rustc_env::BUILD_PROFILE,
        std::env::var(cargo_env::PROFILE).ok().as_deref(),
    );
}

fn generate_logo() {
    let logo_png: PathBuf = constants::LOGO_PNG_PATH.iter().collect();
    println!("cargo::rerun-if-changed={}", logo_png.display());
    let logo_bytes =
        fs::read(&logo_png).unwrap_or_else(|e| panic!("read {}: {e}", logo_png.display()));

    let out_dir = std::env::var_os(cargo_env::OUT_DIR).expect("OUT_DIR is set by cargo");
    let out_path = Path::new(&out_dir).join(constants::LOGO_ANSI_FILE_NAME);
    let ansi = logo_art::image_to_ansi(&logo_bytes, constants::LOGO_ANSI_WIDTH);
    fs::write(&out_path, ansi).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));

    // `include_str!` in src/main.rs reads the file through this variable, so
    // the file name is defined only here.
    let out_path = out_path
        .to_str()
        .unwrap_or_else(|| panic!("OUT_DIR is not UTF-8: {}", out_path.display()));
    emit_rustc_env(rustc_env::LOGO_ANSI_PATH, Some(out_path));
}

fn compile_vendored_grammars() {
    for grammar in constants::VENDORED_GRAMMARS {
        if std::env::var_os(grammar.feature_env).is_none() {
            continue;
        }
        let source_dir = Path::new(constants::VENDOR_DIR)
            .join(grammar.directory)
            .join(constants::GRAMMAR_SOURCE_DIR);
        let mut build = cc::Build::new();
        build.include(&source_dir).warnings(false);
        for source in grammar.sources {
            let path = source_dir.join(source);
            println!("cargo::rerun-if-changed={}", path.display());
            build.file(path);
        }
        build.compile(grammar.library);
    }
}

fn main() {
    build_metadata();
    generate_logo();
    compile_vendored_grammars();
}
