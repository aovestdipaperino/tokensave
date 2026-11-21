//! Checks the build-script constants in `build/constants.rs` (#1069) against
//! the files they name and the literal `env!` keys in `src/main.rs`.

use std::fs;
use std::path::{Path, PathBuf};

#[allow(
    dead_code,
    reason = "the module is shared with build.rs, which uses every item"
)]
#[path = "../build/constants.rs"]
mod constants;

fn package_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn logo_source_exists() {
    let logo: PathBuf = constants::LOGO_PNG_PATH.iter().collect();
    assert!(
        package_root().join(&logo).is_file(),
        "missing logo source {}",
        logo.display()
    );
}

#[test]
fn generated_logo_uses_the_named_file_in_out_dir() {
    let generated = Path::new(env!("TOKENSAVE_BUILD_LOGO_ANSI_PATH"));
    assert_eq!(
        generated.file_name().and_then(|name| name.to_str()),
        Some(constants::LOGO_ANSI_FILE_NAME)
    );
    let ansi = fs::read_to_string(generated).expect("read generated ANSI logo");
    assert!(!ansi.is_empty(), "generated ANSI logo is empty");
}

#[test]
fn main_reads_every_build_variable_by_its_declared_key() {
    let main =
        fs::read_to_string(package_root().join("src").join("main.rs")).expect("read src/main.rs");
    for key in [
        constants::rustc_env::BUILD_COMMIT,
        constants::rustc_env::BUILD_RUSTC,
        constants::rustc_env::BUILD_TARGET,
        constants::rustc_env::BUILD_PROFILE,
        constants::rustc_env::LOGO_ANSI_PATH,
    ] {
        assert!(
            main.contains(&format!("env!(\"{key}\")")),
            "src/main.rs does not read {key} with env!"
        );
    }
}

#[test]
fn vendored_grammar_sources_exist() {
    for grammar in constants::VENDORED_GRAMMARS {
        assert!(
            grammar.feature_env.starts_with("CARGO_FEATURE_LANG_"),
            "{} is not a language feature variable",
            grammar.feature_env
        );
        let source_dir = package_root()
            .join(constants::VENDOR_DIR)
            .join(grammar.directory)
            .join(constants::GRAMMAR_SOURCE_DIR);
        for source in grammar.sources {
            let path = source_dir.join(source);
            assert!(path.is_file(), "missing grammar source {}", path.display());
        }
    }
}
