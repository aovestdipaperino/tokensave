//! How the running tokensave executable was installed (#1055).
//!
//! `tokensave uninstall` and `tokensave purge` remove tokensave's data and
//! integrations but never the executable: Cargo, Homebrew, Scoop and the
//! other installers own it. They end with a reminder, and name an uninstall
//! command only when the running executable carries strong evidence of the
//! installer that put it there:
//!
//! * Homebrew: the canonical path is inside a `Cellar/tokensave/<version>/bin`
//!   keg.
//! * Scoop: the canonical path is `apps/tokensave/<version>/tokensave.exe`
//!   and that version directory holds Scoop's `manifest.json`.
//! * Cargo (including `cargo binstall`): the executable sits directly in
//!   `<CARGO_HOME>/bin` and Cargo's install manifest (`.crates2.json` or
//!   `.crates.toml`) records the `tokensave` package with a `tokensave`
//!   binary.
//!
//! Anything else is [`InstallMethod::Unknown`]. A path that several tools
//! could use is never enough on its own, and detection always looks at the
//! running executable, not the first `tokensave` on `PATH`, so a copied
//! binary or a second installation that shadows the first is reported as
//! unknown rather than mapped to the wrong uninstall command. Nothing is
//! recorded on disk, so there is no stored provenance that could go stale
//! after a binary is copied or reinstalled another way.

use std::path::{Component, Path, PathBuf};

use super::constants::{
    CARGO_BIN_DIR, CARGO_CRATES2_BINS_KEY, CARGO_CRATES2_FILENAME, CARGO_CRATES2_INSTALLS_KEY,
    CARGO_CRATES_FILENAME, CARGO_CRATES_V1_KEY, CARGO_DEFAULT_HOME_DIR, CARGO_HOME_ENV,
    HOMEBREW_BIN_DIR, HOMEBREW_CELLAR_DIR, PACKAGE_NAME, SCOOP_APPS_DIR, SCOOP_MANIFEST_FILENAME,
};

/// The installer that owns the running executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Detects how the running executable was installed.
pub fn detect() -> Provenance {
    let Ok(exe) = std::env::current_exe() else {
        return unknown(None);
    };
    let cargo_home = std::env::var_os(CARGO_HOME_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| crate::agents::home_dir().map(|h| h.join(CARGO_DEFAULT_HOME_DIR)));
    detect_from(&exe, cargo_home.as_deref())
}

/// Detects the installer of `exe`, given Cargo's home directory.
///
/// Pure apart from filesystem reads, so tests can build each layout in a
/// temporary directory.
pub fn detect_from(exe: &Path, cargo_home: Option<&Path>) -> Provenance {
    let canonical = canonical(exe);
    let method = if is_homebrew_keg(&canonical) {
        Some((
            InstallMethod::Homebrew,
            "executable is inside a Homebrew keg",
        ))
    } else if is_scoop_app(&canonical) {
        Some((
            InstallMethod::Scoop,
            "executable is a Scoop app with a manifest",
        ))
    } else if cargo_home.is_some_and(|home| is_cargo_install(&canonical, home)) {
        Some((
            InstallMethod::Cargo,
            "executable is recorded in Cargo's install manifest",
        ))
    } else {
        None
    };
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
    exe.parent()
        .is_some_and(|dir| dir.join(SCOOP_MANIFEST_FILENAME).is_file())
}

/// The executable is `<cargo_home>/bin/tokensave[.exe]` and Cargo's install
/// manifest lists the `tokensave` package with a `tokensave` binary.
fn is_cargo_install(exe: &Path, cargo_home: &Path) -> bool {
    if binary_stem(exe).as_deref() != Some(PACKAGE_NAME) {
        return false;
    }
    let bin_dir = canonical(&cargo_home.join(CARGO_BIN_DIR));
    if exe.parent() != Some(bin_dir.as_path()) {
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
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
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
    let Ok(text) = std::fs::read_to_string(path) else {
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
        touch(&version_dir.join("manifest.json"));
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
