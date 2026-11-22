//! Live installer receipts and the optional versioned record of a verified install.
//!
//! Always check the running executable's own receipts. A `TokenSave` record
//! can corroborate the result but never confers ownership on its own.

use std::io::{Read, Write};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use std::path::{Component, Path, PathBuf};

use super::constants::{
    CARGO_BIN_DIR, CARGO_CRATES2_BINS_KEY, CARGO_CRATES2_FILENAME, CARGO_CRATES2_INSTALLS_KEY,
    CARGO_CRATES_FILENAME, CARGO_CRATES_V1_KEY, CARGO_DEFAULT_HOME_DIR, CARGO_HOME_ENV,
    HOMEBREW_BIN_DIR, HOMEBREW_CELLAR_DIR, PACKAGE_NAME, SCOOP_APPS_DIR, SCOOP_MANIFEST_FILENAME,
};

/// The installer that owns the running executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMethod {
    /// `cargo install` or `cargo binstall`.
    Cargo,
    /// A Homebrew formula keg.
    Homebrew,
    /// A Scoop app.
    Scoop,
    /// No installer could be identified with confidence.
    Unknown,
}

impl InstallMethod {
    /// A short, stable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Homebrew => "homebrew",
            Self::Scoop => "scoop",
            Self::Unknown => "unknown",
        }
    }

    /// The command that removes the executable, when the method is known.
    pub fn uninstall_command(self) -> Option<&'static str> {
        match self {
            Self::Cargo => Some(super::constants::CARGO_UNINSTALL),
            Self::Homebrew => Some(super::constants::HOMEBREW_UNINSTALL),
            Self::Scoop => Some(super::constants::SCOOP_UNINSTALL),
            Self::Unknown => None,
        }
    }
}

/// The detected installation of the running executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub method: InstallMethod,
    /// What the method was concluded from, for `--version --verbose`.
    pub evidence: &'static str,
    /// The running executable, canonicalized when possible.
    pub executable: Option<PathBuf>,
}

pub(super) const INSTALLATION_RECORD: &str = "installation.json";
const INSTALLATION_SCHEMA_VERSION: u32 = 1;
const HOMEBREW_RECEIPT: &str = "INSTALL_RECEIPT.json";
const HOMEBREW_TAP: &str = "aovestdipaperino/tap";
const INSTALLER_METADATA_LIMIT: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct InstallationRecord {
    schema_version: u32,
    method: InstallMethod,
    executable: PathBuf,
    byte_len: u64,
    modified_nanos: u128,
}

impl InstallationRecord {
    fn new(provenance: &Provenance) -> Option<Self> {
        if provenance.method == InstallMethod::Unknown {
            return None;
        }
        let executable = provenance.executable.clone()?;
        let metadata = std::fs::metadata(&executable).ok()?;
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            schema_version: INSTALLATION_SCHEMA_VERSION,
            method: provenance.method,
            executable,
            byte_len: metadata.len(),
            modified_nanos: modified.as_nanos(),
        })
    }
}

fn record_path(home: &Path) -> PathBuf {
    home.join(crate::config::TOKENSAVE_DIR)
        .join(INSTALLATION_RECORD)
}

/// Record verified provenance after a successful global integration install.
/// Installation still works on filesystems where the optional record cannot
/// be created. Detection falls back to the same live receipts in that case.
pub fn record_current_installation(home: &Path) -> std::io::Result<()> {
    let Some(record) = InstallationRecord::new(&detect()) else {
        return Ok(());
    };
    let path = record_path(home);
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.is_file()) {
        return Err(std::io::Error::other(
            "expected a regular installation record",
        ));
    }
    if read_metadata(&path)
        .and_then(|bytes| serde_json::from_slice::<InstallationRecord>(&bytes).ok())
        .as_ref()
        == Some(&record)
    {
        return Ok(());
    }
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut file, &record)?;
    writeln!(file)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn read_metadata(path: &Path) -> Option<Vec<u8>> {
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let mut buffer = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(INSTALLER_METADATA_LIMIT + 1)
        .read_to_end(&mut buffer)
        .ok()?;
    (buffer.len() as u64 <= INSTALLER_METADATA_LIMIT).then_some(buffer)
}

fn json_metadata(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&read_metadata(path)?).ok()
}

/// Detects how the running executable was installed.
pub fn detect() -> Provenance {
    let Ok(exe) = std::env::current_exe() else {
        return unknown(None);
    };
    let cargo_home = std::env::var_os(CARGO_HOME_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| crate::agents::home_dir().map(|h| h.join(CARGO_DEFAULT_HOME_DIR)));
    let mut result = detect_from(&exe, cargo_home.as_deref());
    if result.method != InstallMethod::Unknown {
        if let Some(path) = crate::agents::home_dir().map(|home| record_path(&home)) {
            corroborate_record(&mut result, &path);
        }
    }
    result
}

fn corroborate_record(provenance: &mut Provenance, path: &Path) {
    let Some(stored) = read_metadata(path)
        .and_then(|bytes| serde_json::from_slice::<InstallationRecord>(&bytes).ok())
    else {
        return;
    };
    if InstallationRecord::new(provenance).as_ref() == Some(&stored) {
        provenance.evidence = "recorded and verified against the current installer receipt";
    }
}

/// Detects the installer of `exe`, given Cargo's home directory.
///
/// Pure apart from filesystem reads, so tests can build each layout in a
/// temporary directory.
pub fn detect_from(exe: &Path, cargo_home: Option<&Path>) -> Provenance {
    let Ok(canonical) = exe.canonicalize() else {
        return unknown(Some(exe.to_path_buf()));
    };
    let candidates = [
        (
            is_homebrew_keg(&canonical),
            InstallMethod::Homebrew,
            "Homebrew receipt for aovestdipaperino/tap",
        ),
        (
            is_scoop_app(&canonical),
            InstallMethod::Scoop,
            "matching Scoop app manifest",
        ),
        (
            cargo_home.is_some_and(|home| is_cargo_install(&canonical, home)),
            InstallMethod::Cargo,
            "Cargo install manifest for the running executable",
        ),
    ];
    let mut verified = candidates.into_iter().filter(|(matches, _, _)| *matches);
    let method = verified
        .next()
        .map(|(_, method, evidence)| (method, evidence));
    if verified.next().is_some() {
        return Provenance {
            method: InstallMethod::Unknown,
            evidence: "conflicting installer receipts for the running executable",
            executable: Some(canonical),
        };
    }
    match method {
        Some((method, evidence)) => Provenance {
            method,
            evidence,
            executable: Some(canonical),
        },
        None => unknown(Some(canonical)),
    }
}

fn unknown(executable: Option<PathBuf>) -> Provenance {
    Provenance {
        method: InstallMethod::Unknown,
        evidence: "no installer evidence found for the running executable",
        executable,
    }
}

fn canonical(path: &Path) -> PathBuf {
    crate::state_dir::canonical_root(path)
}

/// The executable's file name with any `.exe` suffix removed.
fn binary_stem(exe: &Path) -> Option<String> {
    let name = exe.file_name()?.to_str()?;
    Some(
        name.strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".EXE"))
            .unwrap_or(name)
            .to_string(),
    )
}

fn normal_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// `.../Cellar/tokensave/<version>/bin/tokensave`.
fn is_homebrew_keg(exe: &Path) -> bool {
    let parts = normal_components(exe);
    let n = parts.len();
    n >= 5
        && parts[n - 5] == HOMEBREW_CELLAR_DIR
        && parts[n - 4] == PACKAGE_NAME
        && parts[n - 2] == HOMEBREW_BIN_DIR
        && parts[n - 1] == PACKAGE_NAME
        && exe.parent().and_then(Path::parent).is_some_and(|keg| {
            json_metadata(&keg.join(HOMEBREW_RECEIPT))
                .and_then(|receipt| {
                    receipt
                        .pointer("/source/tap")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .as_deref()
                == Some(HOMEBREW_TAP)
        })
}

/// `.../apps/tokensave/<version>/tokensave.exe`, with Scoop's manifest next
/// to it. The canonical path resolves Scoop's `current` junction.
fn is_scoop_app(exe: &Path) -> bool {
    let parts = normal_components(exe);
    let n = parts.len();
    if n < 4
        || !parts[n - 4].eq_ignore_ascii_case(SCOOP_APPS_DIR)
        || !parts[n - 3].eq_ignore_ascii_case(PACKAGE_NAME)
        || binary_stem(exe).as_deref() != Some(PACKAGE_NAME)
    {
        return false;
    }
    exe.parent().is_some_and(|dir| {
        json_metadata(&dir.join(SCOOP_MANIFEST_FILENAME)).is_some_and(|manifest| {
            manifest["version"].as_str() == Some(parts[n - 2].as_str())
                && scoop_lists_binary(&manifest["bin"])
        })
    })
}

fn scoop_lists_binary(bins: &serde_json::Value) -> bool {
    if let Some(bin) = bins.as_str() {
        return bin.eq_ignore_ascii_case("tokensave.exe");
    }
    bins.as_array().is_some_and(|bins| {
        bins.iter().any(|bin| {
            bin.as_str()
                .or_else(|| bin.get(0).and_then(serde_json::Value::as_str))
                .is_some_and(|bin| bin.eq_ignore_ascii_case("tokensave.exe"))
        })
    })
}

/// The executable is `<cargo_home>/bin/tokensave[.exe]` and Cargo's install
/// manifest lists the `tokensave` package with a `tokensave` binary.
fn is_cargo_install(exe: &Path, cargo_home: &Path) -> bool {
    if binary_stem(exe).as_deref() != Some(PACKAGE_NAME) {
        return false;
    }
    let bin_dir = canonical(&cargo_home.join(CARGO_BIN_DIR))
        .canonicalize()
        .ok();
    if exe.parent() != bin_dir.as_deref() {
        return false;
    }
    crates2_lists_tokensave(&cargo_home.join(CARGO_CRATES2_FILENAME))
        || crates_toml_lists_tokensave(&cargo_home.join(CARGO_CRATES_FILENAME))
}

/// Whether a package id (`"tokensave 7.17.0 (registry+...)"`) names tokensave.
fn is_tokensave_package_id(id: &str) -> bool {
    id.split_whitespace().next() == Some(PACKAGE_NAME)
}

fn names_tokensave_binary(bin: &str) -> bool {
    binary_stem(Path::new(bin)).as_deref() == Some(PACKAGE_NAME)
}

fn crates2_lists_tokensave(path: &Path) -> bool {
    let Some(json) = json_metadata(path) else {
        return false;
    };
    let Some(installs) = json
        .get(CARGO_CRATES2_INSTALLS_KEY)
        .and_then(|v| v.as_object())
    else {
        return false;
    };
    installs.iter().any(|(id, entry)| {
        is_tokensave_package_id(id)
            && entry
                .get(CARGO_CRATES2_BINS_KEY)
                .and_then(|b| b.as_array())
                .is_some_and(|bins| {
                    bins.iter()
                        .filter_map(|b| b.as_str())
                        .any(names_tokensave_binary)
                })
    })
}

fn crates_toml_lists_tokensave(path: &Path) -> bool {
    let Some(bytes) = read_metadata(path) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return false;
    };
    let Some(v1) = table.get(CARGO_CRATES_V1_KEY).and_then(|v| v.as_table()) else {
        return false;
    };
    v1.iter().any(|(id, bins)| {
        is_tokensave_package_id(id)
            && bins.as_array().is_some_and(|bins| {
                bins.iter()
                    .filter_map(|b| b.as_str())
                    .any(names_tokensave_binary)
            })
    })
}

/// The reminder `uninstall` and `purge` print last: the executable stays
/// installed, and how to remove it.
pub fn removal_reminder(provenance: &Provenance) -> String {
    let location = provenance
        .executable
        .as_ref()
        .map_or_else(String::new, |p| format!(" at {}", p.display()));
    match provenance.method.uninstall_command() {
        Some(command) => format!(
            "The tokensave executable is still installed{location}. \
             Remove it with: {command}"
        ),
        None => format!(
            "The tokensave executable is still installed{location}. \
             Its installer could not be identified, so remove it with the tool you \
             installed it with, or delete the file if you copied it there yourself."
        ),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;

    fn exe_name() -> &'static str {
        if cfg!(windows) {
            "tokensave.exe"
        } else {
            "tokensave"
        }
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    fn cargo_layout(manifest: Option<(&str, &str)>) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let cargo_home = tmp.path().join("cargo");
        let exe = cargo_home.join("bin").join(exe_name());
        touch(&exe);
        if let Some((name, contents)) = manifest {
            fs::write(cargo_home.join(name), contents).unwrap();
        }
        (tmp, cargo_home, exe)
    }

    #[test]
    fn cargo_install_recorded_in_crates2_json_is_cargo() {
        let (_tmp, home, exe) = cargo_layout(Some((
            ".crates2.json",
            r#"{"installs":{"tokensave 7.17.0 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["tokensave"]}}}"#,
        )));
        let p = detect_from(&exe, Some(&home));
        assert_eq!(p.method, InstallMethod::Cargo);
        assert_eq!(
            p.method.uninstall_command(),
            Some("cargo uninstall tokensave")
        );
    }

    #[test]
    fn cargo_install_recorded_in_crates_toml_is_cargo() {
        let (_tmp, home, exe) = cargo_layout(Some((
            ".crates.toml",
            "[v1]\n\"tokensave 7.17.0 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"tokensave\"]\n",
        )));
        assert_eq!(detect_from(&exe, Some(&home)).method, InstallMethod::Cargo);
    }

    /// A binary copied into `~/.cargo/bin` by hand has no manifest entry.
    #[test]
    fn copied_binary_in_cargo_bin_is_unknown() {
        let (_tmp, home, exe) = cargo_layout(None);
        assert_eq!(
            detect_from(&exe, Some(&home)).method,
            InstallMethod::Unknown
        );
        let (_tmp, home, exe) = cargo_layout(Some((
            ".crates2.json",
            r#"{"installs":{"tokensave-lite 1.0.0 (path+file:///x)":{"bins":["other"]}}}"#,
        )));
        assert_eq!(
            detect_from(&exe, Some(&home)).method,
            InstallMethod::Unknown
        );
    }

    /// Cargo records tokensave, but the running copy lives elsewhere (a
    /// second installation earlier on PATH): the running one decides.
    #[test]
    fn running_executable_outside_cargo_bin_is_unknown_even_when_cargo_lists_it() {
        let (tmp, home, _exe) = cargo_layout(Some((
            ".crates2.json",
            r#"{"installs":{"tokensave 7.17.0 (registry+x)":{"bins":["tokensave"]}}}"#,
        )));
        let other = tmp.path().join("opt").join("bin").join(exe_name());
        touch(&other);
        assert_eq!(
            detect_from(&other, Some(&home)).method,
            InstallMethod::Unknown
        );
    }

    #[test]
    fn homebrew_keg_is_homebrew() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp
            .path()
            .join("opt")
            .join("homebrew")
            .join("Cellar")
            .join("tokensave")
            .join("7.17.0")
            .join("bin")
            .join("tokensave");
        touch(&exe);
        assert_eq!(detect_from(&exe, None).method, InstallMethod::Unknown);
        let keg = exe.parent().and_then(Path::parent).expect("keg");
        fs::write(
            keg.join(HOMEBREW_RECEIPT),
            r#"{"source":{"tap":"aovestdipaperino/tap"}}"#,
        )
        .unwrap();
        let p = detect_from(&exe, None);
        assert_eq!(p.method, InstallMethod::Homebrew);
        assert_eq!(
            p.method.uninstall_command(),
            Some("brew uninstall tokensave")
        );
    }

    /// A `homebrew` directory alone is not evidence: anything can live there.
    #[test]
    fn homebrew_prefix_bin_without_keg_is_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp
            .path()
            .join("opt")
            .join("homebrew")
            .join("bin")
            .join("tokensave");
        touch(&exe);
        assert_eq!(detect_from(&exe, None).method, InstallMethod::Unknown);
    }

    #[test]
    fn scoop_app_with_manifest_is_scoop() {
        let tmp = tempfile::tempdir().unwrap();
        let version_dir = tmp
            .path()
            .join("scoop")
            .join("apps")
            .join("tokensave")
            .join("7.17.0");
        let exe = version_dir.join("tokensave.exe");
        touch(&exe);
        fs::write(
            version_dir.join("manifest.json"),
            r#"{"version":"7.17.0","bin":"tokensave.exe"}"#,
        )
        .unwrap();
        let p = detect_from(&exe, None);
        assert_eq!(p.method, InstallMethod::Scoop);
        assert_eq!(
            p.method.uninstall_command(),
            Some("scoop uninstall tokensave")
        );
    }

    #[test]
    fn scoop_shaped_path_without_manifest_is_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp
            .path()
            .join("scoop")
            .join("apps")
            .join("tokensave")
            .join("7.17.0")
            .join("tokensave.exe");
        touch(&exe);
        assert_eq!(detect_from(&exe, None).method, InstallMethod::Unknown);
    }

    #[test]
    fn release_archive_binary_is_unknown_and_reminder_names_no_command() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("tools").join(exe_name());
        touch(&exe);
        let p = detect_from(&exe, None);
        assert_eq!(p.method, InstallMethod::Unknown);
        let reminder = removal_reminder(&p);
        assert!(reminder.contains("still installed"));
        assert!(!reminder.contains("cargo uninstall"));
        assert!(!reminder.contains("brew uninstall"));
        assert!(!reminder.contains("scoop uninstall"));
    }

    #[test]
    fn record_corroborates_only_the_same_live_install() {
        let (tmp, cargo_home, exe) = cargo_layout(Some((
            ".crates2.json",
            r#"{"installs":{"tokensave 7.17.0 (registry+x)":{"bins":["tokensave"]}}}"#,
        )));
        let mut verified = detect_from(&exe, Some(&cargo_home));
        let record = InstallationRecord::new(&verified).expect("verified receipt");
        let path = tmp.path().join(INSTALLATION_RECORD);
        fs::write(&path, serde_json::to_vec(&record).expect("record")).unwrap();
        corroborate_record(&mut verified, &path);
        assert!(verified.evidence.starts_with("recorded"));

        fs::write(&exe, "replacement").unwrap();
        let mut stale = detect_from(&exe, Some(&cargo_home));
        corroborate_record(&mut stale, &path);
        assert!(!stale.evidence.starts_with("recorded"));

        let copy = tmp.path().join(exe_name());
        fs::copy(&exe, &copy).unwrap();
        let mut copied = detect_from(&copy, Some(&cargo_home));
        corroborate_record(&mut copied, &path);
        assert_eq!(copied.method, InstallMethod::Unknown);

        fs::remove_file(cargo_home.join(CARGO_CRATES2_FILENAME)).unwrap();
        let mut missing_receipt = detect_from(&exe, Some(&cargo_home));
        corroborate_record(&mut missing_receipt, &path);
        assert_eq!(missing_receipt.method, InstallMethod::Unknown);
    }

    #[test]
    fn reminder_names_the_command_when_known() {
        let p = Provenance {
            method: InstallMethod::Cargo,
            evidence: "test",
            executable: None,
        };
        assert_eq!(
            removal_reminder(&p),
            "The tokensave executable is still installed. \
             Remove it with: cargo uninstall tokensave"
        );
    }
}
