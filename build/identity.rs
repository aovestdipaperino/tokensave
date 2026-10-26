use std::{env, fs, path::PathBuf};

pub fn configure() {
    println!("cargo::rerun-if-changed=build/identity.json");
    println!("cargo::rerun-if-changed=build/identity.rs");
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_BETA");
    println!("cargo::rerun-if-env-changed=TOKENSAVE_BETA_VERSION");
    let package_version = env::var("CARGO_PKG_VERSION").unwrap();
    let beta = env::var_os("CARGO_FEATURE_BETA").is_some() || package_version.contains('-');
    let config: serde_json::Value = serde_json::from_str(include_str!("identity.json")).unwrap();
    let identity = &config[if beta { "beta" } else { "stable" }];
    let mut version = if beta && !package_version.contains('-') {
        format!(
            "{package_version}-{}",
            identity["version_suffix"].as_str().unwrap()
        )
    } else {
        package_version.clone()
    };
    if let Ok(requested) = env::var("TOKENSAVE_BETA_VERSION") {
        assert!(beta, "TOKENSAVE_BETA_VERSION requires a beta build");
        let base = package_version.split('-').next().unwrap();
        let prefix = format!("{base}-beta.");
        let counter = requested.strip_prefix(&prefix).unwrap_or("");
        assert!(
            !counter.is_empty()
                && counter.bytes().all(|c| c.is_ascii_digit())
                && (counter == "0" || !counter.starts_with('0')),
            "beta version must be {base}-beta.N (N is a nonnegative integer)"
        );
        version = requested;
    }
    println!("cargo::rustc-env=TOKENSAVE_BUILD_VERSION={version}");
    println!("cargo::rustc-env=TOKENSAVE_BUILD_BETA={beta}");
    let long_version = if beta {
        format!("{} {version}", identity["product_name"].as_str().unwrap())
    } else {
        version.clone()
    };
    println!("cargo::rustc-env=TOKENSAVE_LONG_VERSION={long_version}");
    for (key, variable) in [
        ("product_name", "TOKENSAVE_PRODUCT_NAME"),
        ("cli_name", "TOKENSAVE_CLI_NAME"),
        ("application_id", "TOKENSAVE_APPLICATION_ID"),
    ] {
        println!(
            "cargo::rustc-env={variable}={}",
            identity[key].as_str().unwrap()
        );
    }
    for (channel, prefix) in [("beta", "BETA"), ("stable", "STABLE")] {
        for platform in ["windows", "unix"] {
            let value = config[channel][format!("{platform}_executable")]
                .as_str()
                .unwrap();
            println!(
                "cargo::rustc-env=TOKENSAVE_{prefix}_{}_EXECUTABLE={value}",
                platform.to_uppercase()
            );
        }
    }

    // Stable has never embedded Windows resources; preserve those exact defaults.
    if !beta || env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }
    let icon = identity["icon"].as_str().unwrap();
    println!("cargo::rerun-if-changed={icon}");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let icon = root.join(icon).to_string_lossy().replace('\\', "/");
    let numeric = package_version.split(['-', '+']).next().unwrap();
    let numeric = numeric.replace('.', ",");
    let product = identity["product_name"].as_str().unwrap();
    let executable = identity["windows_executable"].as_str().unwrap();
    let internal_name = identity["cli_name"].as_str().unwrap();
    let resource = format!(
        r#"
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric},0
PRODUCTVERSION {numeric},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x2L
FILEOS 0x40004L
FILETYPE 0x1L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "FileDescription", "{product}\0"
      VALUE "ProductName", "{product}\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "ProductVersion", "{version}\0"
      VALUE "InternalName", "{internal_name}\0"
      VALUE "OriginalFilename", "{executable}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#
    );
    let path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("identity.rc");
    fs::write(&path, resource).unwrap();
    embed_resource::compile(&path, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
