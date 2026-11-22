//! Exercise the CLI with a disposable managed installation.

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn cargo_upgrade_and_channel_switch_preserve_the_managed_executable() {
    let fixture = tempfile::tempdir().expect("fixture");
    let home = fixture.path();
    let root = home.join("cargo-root");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("Cargo bin");
    let binary = bin.join(format!("tokensave{}", std::env::consts::EXE_SUFFIX));
    let source = Path::new(env!("CARGO_BIN_EXE_tokensave"));
    // Give the running executable its own install path and independent contents.
    fs::copy(source, &binary).expect("copy test binary");
    let manifest = root.join(".crates2.json");
    let id = format!(
        "tokensave {} (registry+https://github.com/rust-lang/crates.io-index)",
        env!("CARGO_PKG_VERSION")
    );
    let receipt = serde_json::json!({"installs": { (id): {
        "bins": [binary.file_name().expect("executable name").to_string_lossy()],
        "profile": "release",
        "features": [],
        "all_features": false,
        "no_default_features": false
    }}})
    .to_string();
    fs::write(&manifest, &receipt).expect("write receipt");
    let original = fs::metadata(&binary).expect("binary metadata");
    let run = |args: &[&str]| {
        Command::new(&binary)
            .args(args)
            .current_dir(home)
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("CARGO_HOME", &root)
            .output()
            .expect("run TokenSave")
    };

    for args in [
        &["upgrade", "--dry-run"][..],
        &["upgrade", "--standalone", "--kill"][..],
    ] {
        let output = run(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        if tokensave::cloud::is_beta() {
            assert!(
                stderr.contains("Repeat the original cargo install command"),
                "{stderr}"
            );
        } else {
            assert!(
                stderr.contains("cargo install tokensave --locked --force"),
                "{stderr}"
            );
        }
    }
    let other_channel = if tokensave::cloud::is_beta() {
        "stable"
    } else {
        "beta"
    };
    let output = run(&["channel", other_channel, "--standalone"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("Cannot switch this installation in place"));
    let final_metadata = fs::metadata(binary).expect("managed executable");
    assert_eq!(final_metadata.len(), original.len());
    assert_eq!(final_metadata.modified().ok(), original.modified().ok());
    assert_eq!(fs::read_to_string(manifest).expect("receipt"), receipt);
    assert!(!home.join(".tokensave").exists());
}
