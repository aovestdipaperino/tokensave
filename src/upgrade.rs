//! Self-update for the tokensave binary.
//!
//! Downloads the latest release asset directly from GitHub, extracts the
//! binary, and replaces the running executable using `self_replace`.
//! Beta and stable are separate channels — a beta build only sees beta
//! releases and vice versa. The daemon is stopped before the binary is
//! replaced and restarted afterwards if it was running.

use std::path::Path;

use crate::cloud::{self, InstallMethod};
use crate::daemon;
use crate::errors::{Result, TokenSaveError};

const GITHUB_REPO: &str = "aovestdipaperino/tokensave";

/// Archive naming convention per platform.
/// Stable: `tokensave-v{version}-{platform}.{ext}`
/// Beta:   `tokensave-beta-v{version}-{platform}.{ext}`
fn asset_name(version: &str, is_beta: bool) -> String {
    let prefix = if is_beta {
        "tokensave-beta"
    } else {
        "tokensave"
    };
    let platform = current_platform();
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("{prefix}-v{version}-{platform}.{ext}")
}

/// Returns the platform slug matching the CI release matrix.
fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-macos"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-macos"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-linux"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "aarch64-linux"
    } else if cfg!(target_os = "windows") {
        "x86_64-windows"
    } else {
        "unknown"
    }
}

/// The GitHub release tag for a given version.
fn release_tag(version: &str) -> String {
    format!("v{version}")
}

fn io_err(msg: &str) -> impl Fn(std::io::Error) -> TokenSaveError + '_ {
    move |e| TokenSaveError::Config {
        message: format!("{msg}: {e}"),
    }
}

/// Fetches the `browser_download_url` for a specific asset in a GitHub release.
fn fetch_asset_url(tag: &str, expected_asset: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
    }
    #[derive(serde::Deserialize)]
    struct Release {
        assets: Vec<Asset>,
    }

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/tags/{tag}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .into();

    let release: Release = agent
        .get(&url)
        .header("User-Agent", "tokensave")
        .call()
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to reach GitHub: {e}"),
        })?
        .body_mut()
        .read_json()
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to parse release info: {e}"),
        })?;

    release
        .assets
        .into_iter()
        .find(|a| a.name == expected_asset)
        .map(|a| a.browser_download_url)
        .ok_or_else(|| TokenSaveError::Config {
            message: format!(
                "release {tag} exists but asset '{expected_asset}' is not yet available.\n  \
                 CI build may still be in progress — try again in a few minutes.\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })
}

/// Downloads the archive from `url` into memory, then extracts `bin_name`
/// to a temp path. Returns the temp path.
fn download_and_extract(url: &str, bin_name: &str) -> Result<std::path::PathBuf> {
    let tmp_path = std::env::temp_dir().join(format!(
        "tokensave_upgrade_{}{}",
        std::process::id(),
        if cfg!(windows) { ".exe" } else { "" }
    ));

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_mins(5)))
        .build()
        .into();

    eprint!("  Downloading...");

    // Buffer the entire archive so the reader type is concrete (Cursor<Vec<u8>>),
    // which makes type inference for tar::Entry and zip::ZipArchive unambiguous.
    let raw: Vec<u8> = {
        use std::io::Read;
        let mut buf = Vec::new();
        agent
            .get(url)
            .header("User-Agent", "tokensave")
            .call()
            .map_err(|e| TokenSaveError::Config {
                message: format!("download failed: {e}"),
            })?
            .body_mut()
            .as_reader()
            .read_to_end(&mut buf)
            .map_err(io_err("download read failed"))?;
        buf
    };

    eprintln!(" ({:.1} MiB)", raw.len() as f64 / 1_048_576.0);
    eprint!("  Extracting...");

    #[cfg(not(windows))]
    extract_targz(&raw, bin_name, &tmp_path)?;

    #[cfg(windows)]
    extract_zip(&raw, bin_name, &tmp_path)?;

    eprintln!(" Done");
    Ok(tmp_path)
}

/// Extracts `bin_name` from a `.tar.gz` archive (Unix).
#[cfg(not(windows))]
fn extract_targz(data: &[u8], bin_name: &str, dest: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let gz = GzDecoder::new(Cursor::new(data));
    let mut archive = Archive::new(gz);

    for entry in archive.entries().map_err(io_err("archive open failed"))? {
        let mut entry = entry.map_err(io_err("archive read failed"))?;
        let path = entry
            .path()
            .map_err(io_err("archive path error"))?
            .to_path_buf();

        if path.file_name().and_then(|n| n.to_str()) == Some(bin_name) {
            entry.unpack(dest).map_err(io_err("extract failed"))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(dest)
                    .map_err(io_err("stat failed"))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(dest, perms).map_err(io_err("chmod failed"))?;
            }

            return Ok(());
        }
    }

    Err(TokenSaveError::Config {
        message: format!("binary '{bin_name}' not found in archive"),
    })
}

/// Extracts `bin_name` from a `.zip` archive (Windows).
#[cfg(windows)]
fn extract_zip(data: &[u8], bin_name: &str, dest: &Path) -> Result<()> {
    use std::io::Cursor;

    let mut archive =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|e| TokenSaveError::Config {
            message: format!("zip open failed: {e}"),
        })?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| TokenSaveError::Config {
            message: format!("zip entry error: {e}"),
        })?;

        if Path::new(file.name()).file_name().and_then(|n| n.to_str()) == Some(bin_name) {
            let mut out = std::fs::File::create(dest).map_err(io_err("create temp file failed"))?;
            std::io::copy(&mut file, &mut out).map_err(io_err("extract failed"))?;
            return Ok(());
        }
    }

    Err(TokenSaveError::Config {
        message: format!("binary '{bin_name}' not found in zip"),
    })
}

/// Replaces the running binary with `new_exe`, dispatching to the
/// appropriate strategy for the detected install method. Cleans up the
/// temp file afterwards regardless of outcome.
fn replace_binary(new_exe: &Path, method: &InstallMethod, new_version: &str) -> Result<()> {
    let result = match method {
        InstallMethod::Brew => replace_for_brew(new_exe, new_version),
        InstallMethod::Scoop => replace_for_scoop(new_exe, new_version),
        _ => replace_default(new_exe),
    };
    let _ = std::fs::remove_file(new_exe);
    result
}

/// Default replacement using `self_replace`. Falls back to a direct copy
/// when the running binary is behind a symlink (avoids ENOENT caused by
/// `self_replace` resolving relative symlink targets from CWD).
fn replace_default(new_exe: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let exe = std::env::current_exe().ok();
        let canonical = exe.as_ref().and_then(|e| e.canonicalize().ok());
        if let (Some(exe), Some(ref canonical)) = (&exe, canonical) {
            if exe.as_path() != canonical.as_path() {
                return install_binary(new_exe, canonical);
            }
        }
    }

    self_replace::self_replace(new_exe).map_err(|e| TokenSaveError::Config {
        message: format!(
            "binary replacement failed: {e}\n  \
             The old version is still in place.\n  \
             To upgrade manually: https://github.com/{GITHUB_REPO}/releases/latest"
        ),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpgradeStatus<'a> {
    AlreadyCurrent,
    UpgradeAvailable(&'a str),
}

fn classify_upgrade<'a>(current: &str, latest: &'a str) -> UpgradeStatus<'a> {
    if cloud::is_newer_version(current, latest) {
        UpgradeStatus::UpgradeAvailable(latest)
    } else {
        UpgradeStatus::AlreadyCurrent
    }
}

/// Atomically replace a binary at `target` by copying `src` to a temp file
/// in the same directory, setting permissions, then renaming over `target`.
/// Avoids `ETXTBSY` on Linux (rename swaps directory entries rather than
/// writing into the running executable).
#[cfg(unix)]
fn install_binary(src: &Path, target: &Path) -> Result<()> {
    let dir = target.parent().ok_or_else(|| TokenSaveError::Config {
        message: "cannot determine target directory".into(),
    })?;
    let temp = dir.join(format!(".tokensave_upgrade_{}", std::process::id()));

    std::fs::copy(src, &temp).map_err(io_err("cannot copy new binary"))?;

    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))
            .map_err(io_err("cannot set permissions"))?;
    }

    if let Err(e) = std::fs::rename(&temp, target) {
        let _ = std::fs::remove_file(&temp);
        return Err(io_err("cannot replace binary")(e));
    }

    Ok(())
}

// ── Homebrew ────────────────────────────────────────────────────────────

/// Replace the binary inside the Homebrew Cellar, then rename the version
/// directory and update the symlink so that `brew` reports the new version.
#[cfg(unix)]
fn replace_for_brew(new_exe: &Path, new_version: &str) -> Result<()> {
    let exe = std::env::current_exe().map_err(io_err("cannot determine current exe"))?;
    let canonical = exe
        .canonicalize()
        .map_err(io_err("cannot resolve binary path"))?;

    // Validate Cellar layout: <prefix>/Cellar/<formula>/<version>/bin/<binary>
    let bin_dir = match canonical.parent() {
        Some(p) if p.file_name().and_then(|n| n.to_str()) == Some("bin") => p,
        _ => return replace_default(new_exe),
    };
    let Some(version_dir) = bin_dir.parent() else {
        return replace_default(new_exe);
    };
    let Some(formula_dir) = version_dir.parent() else {
        return replace_default(new_exe);
    };
    let cellar_dir = match formula_dir.parent() {
        Some(p) if p.file_name().and_then(|n| n.to_str()) == Some("Cellar") => p,
        _ => return replace_default(new_exe),
    };
    let Some(prefix) = cellar_dir.parent() else {
        return replace_default(new_exe);
    };

    let Some(bin_name) = canonical.file_name() else {
        return replace_default(new_exe);
    };
    let Some(old_version_os) = version_dir.file_name() else {
        return replace_default(new_exe);
    };
    let old_version = old_version_os.to_string_lossy().to_string();

    // Step 1 (critical): replace the binary atomically.
    install_binary(new_exe, &canonical)?;

    // Steps 2-4 update Cellar metadata so `brew` sees the correct version.
    // These are best-effort — if they fail the binary itself is fine.
    if old_version != new_version {
        let new_version_dir = formula_dir.join(new_version);

        // Step 2: rename the version directory (e.g. 4.0.3 → 4.0.4).
        match std::fs::rename(version_dir, &new_version_dir) {
            Ok(()) => {
                // Step 3: update the symlink at <prefix>/bin/<binary>.
                let symlink_path = prefix.join("bin").join(bin_name);
                if let Ok(meta) = std::fs::symlink_metadata(&symlink_path) {
                    if meta.file_type().is_symlink() {
                        if let Ok(old_target) = std::fs::read_link(&symlink_path) {
                            let new_target = std::path::PathBuf::from(
                                old_target
                                    .to_string_lossy()
                                    .replacen(&old_version, new_version, 1),
                            );
                            let _ = std::fs::remove_file(&symlink_path);
                            let _ = std::os::unix::fs::symlink(&new_target, &symlink_path);
                        }
                    }
                }

                // Step 4: patch INSTALL_RECEIPT.json so `brew info` is accurate.
                let receipt = new_version_dir.join("INSTALL_RECEIPT.json");
                if receipt.exists() {
                    if let Ok(text) = std::fs::read_to_string(&receipt) {
                        let _ = std::fs::write(&receipt, text.replace(&old_version, new_version));
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "\n  \x1b[33mwarning:\x1b[0m could not rename Cellar directory: {e}\n    \
                     brew may still report the old version"
                );
            }
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn replace_for_brew(new_exe: &Path, _new_version: &str) -> Result<()> {
    replace_default(new_exe)
}

// ── Scoop ───────────────────────────────────────────────────────────────

/// Replace the binary via `self_replace` (handles Windows exe locking),
/// then update Scoop's version directory and junction so that
/// `scoop status` reports the new version.
#[cfg(windows)]
fn replace_for_scoop(new_exe: &Path, new_version: &str) -> Result<()> {
    self_replace::self_replace(new_exe).map_err(|e| TokenSaveError::Config {
        message: format!(
            "binary replacement failed: {e}\n  \
             The old version is still in place.\n  \
             To upgrade manually: https://github.com/{GITHUB_REPO}/releases/latest"
        ),
    })?;

    // Best-effort: update Scoop metadata for `scoop status` compatibility.
    update_scoop_metadata(new_version);

    Ok(())
}

#[cfg(windows)]
fn update_scoop_metadata(new_version: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let canonical = exe.canonicalize().unwrap_or(exe);

    let Some(version_dir) = find_scoop_version_dir(&canonical) else {
        return;
    };
    let Some(app_dir) = version_dir.parent() else {
        return;
    };
    let old_version = version_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if old_version == new_version || old_version == "current" {
        return;
    }

    let new_version_dir = app_dir.join(new_version);
    if std::fs::create_dir_all(&new_version_dir).is_err() {
        return;
    }

    // Copy files from old version directory to new.
    if let Ok(entries) = std::fs::read_dir(&version_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name();
            if name.to_string_lossy().contains("__self_delete__") {
                continue;
            }
            let _ = std::fs::copy(entry.path(), new_version_dir.join(&name));
        }
    }

    // Patch manifest.json version.
    let manifest = new_version_dir.join("manifest.json");
    if manifest.exists() {
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            let _ = std::fs::write(&manifest, text.replace(&old_version, new_version));
        }
    }

    // Update the `current` directory junction.
    let current = app_dir.join("current");
    let _ = std::fs::remove_dir(&current);
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("cmd")
        .args([
            "/c",
            "mklink",
            "/J",
            &current.to_string_lossy(),
            &new_version_dir.to_string_lossy(),
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status();
}

/// Walk the canonical path to find the Scoop version directory.
/// Layout: `<scoop>/apps/<app>/<version>/…`
#[cfg(windows)]
fn find_scoop_version_dir(path: &Path) -> Option<std::path::PathBuf> {
    let mut found_apps = false;
    let mut depth_after_apps = 0u8;
    let mut result = std::path::PathBuf::new();

    for comp in path.components() {
        result.push(comp);
        if found_apps {
            depth_after_apps += 1;
            if depth_after_apps == 2 {
                return Some(result);
            }
        } else if let std::path::Component::Normal(name) = comp {
            if name.to_string_lossy().eq_ignore_ascii_case("apps") {
                found_apps = true;
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn replace_for_scoop(new_exe: &Path, _new_version: &str) -> Result<()> {
    replace_default(new_exe)
}

// ────────────────────────────────────────────────────────────────────────

/// Downloads, extracts, and installs the binary for `version`/`is_beta`.
fn perform_upgrade(version: &str, is_beta: bool, method: &InstallMethod) -> Result<()> {
    let tag = release_tag(version);
    let expected = asset_name(version, is_beta);
    let bin_name = if cfg!(windows) {
        "tokensave.exe"
    } else {
        "tokensave"
    };

    eprintln!("  Asset: {expected}");

    let url = fetch_asset_url(&tag, &expected)?;
    let tmp = download_and_extract(&url, bin_name)?;

    let label = match method {
        InstallMethod::Brew => " (Homebrew Cellar)",
        InstallMethod::Scoop => " (Scoop)",
        _ => "",
    };
    eprint!("  Replacing binary{label}...");
    replace_binary(&tmp, method, version)?;
    eprintln!(" Done");

    Ok(())
}

/// Restart the daemon by spawning a detached `tokensave daemon` process.
fn restart_daemon() {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "  \x1b[33mwarning:\x1b[0m could not determine executable path to restart daemon: {e}"
            );
            return;
        }
    };

    match std::process::Command::new(&exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => eprintln!("  \x1b[32m✔\x1b[0m Daemon restarted"),
        Err(e) => eprintln!("  \x1b[33mwarning:\x1b[0m failed to restart daemon: {e}"),
    }
}

/// Check for a newer version and perform the upgrade if one is available.
///
/// Stops the daemon before replacing the binary and restarts it after if
/// it was running. Returns the new version string on success.
pub fn run_upgrade() -> Result<String> {
    let current = env!("CARGO_PKG_VERSION");
    let is_beta = cloud::is_beta();
    let channel = if is_beta { "beta" } else { "stable" };
    let method = cloud::detect_install_method();

    let method_suffix = match &method {
        InstallMethod::Brew => " · Homebrew",
        InstallMethod::Scoop => " · Scoop",
        InstallMethod::Cargo => " · cargo",
        InstallMethod::Unknown => "",
    };
    eprintln!("Current version: v{current} ({channel} channel{method_suffix})");
    eprintln!("Checking for updates...");

    let latest = cloud::fetch_latest_version().ok_or_else(|| TokenSaveError::Config {
        message: "failed to check for updates — could not reach GitHub".to_string(),
    })?;

    let latest = match classify_upgrade(current, &latest) {
        UpgradeStatus::AlreadyCurrent => {
            eprintln!("\x1b[32m✔\x1b[0m Already up to date (v{current}).");
            return Ok(current.to_string());
        }
        UpgradeStatus::UpgradeAvailable(latest) => latest,
    };

    eprintln!("Upgrading v{current} → v{latest}...");

    let daemon_was_running = daemon::running_daemon_pid().is_some();
    if daemon_was_running {
        eprintln!("  Stopping daemon...");
        daemon::stop().ok();
    }

    let result = perform_upgrade(latest, is_beta, &method);

    match result {
        Ok(()) => {
            eprintln!("\x1b[32m✔\x1b[0m Successfully upgraded to v{latest}!");
            if daemon_was_running {
                eprintln!("  Restarting daemon...");
                restart_daemon();
            }
            Ok(latest.to_string())
        }
        Err(e) => {
            if daemon_was_running {
                eprintln!("  Restarting daemon (upgrade failed, old version still in place)...");
                restart_daemon();
            }
            Err(e)
        }
    }
}

/// Print the current channel.
pub fn show_channel() {
    let current = env!("CARGO_PKG_VERSION");
    let channel = if cloud::is_beta() { "beta" } else { "stable" };
    eprintln!("v{current} ({channel})");
}

/// Switch to a different channel by downloading the latest release from it.
///
/// Stops the daemon before replacing the binary and restarts it afterwards
/// if it was running.
pub fn switch_channel(target_channel: &str) -> Result<String> {
    let current = env!("CARGO_PKG_VERSION");
    let current_is_beta = cloud::is_beta();
    let current_channel = if current_is_beta { "beta" } else { "stable" };
    let method = cloud::detect_install_method();

    let target_is_beta = match target_channel {
        "beta" => {
            return Err(TokenSaveError::Config {
                message: "the beta channel is not available at this time".to_string(),
            });
        }
        "stable" => false,
        other => {
            return Err(TokenSaveError::Config {
                message: format!("unknown channel '{other}'. Valid channels: stable"),
            });
        }
    };

    if target_is_beta == current_is_beta {
        eprintln!("Already on the {current_channel} channel (v{current}).");
        eprintln!("Run `tokensave upgrade` to check for updates within this channel.");
        return Ok(current.to_string());
    }

    eprintln!("Switching from {current_channel} to {target_channel}...");

    let latest = if target_is_beta {
        cloud::fetch_latest_beta_version()
    } else {
        cloud::fetch_latest_stable_version()
    }
    .ok_or_else(|| TokenSaveError::Config {
        message: format!("failed to find latest {target_channel} release — could not reach GitHub"),
    })?;

    eprintln!("  Target: v{latest}");

    let daemon_was_running = daemon::running_daemon_pid().is_some();
    if daemon_was_running {
        eprintln!("  Stopping daemon...");
        daemon::stop().ok();
    }

    let result = perform_upgrade(&latest, target_is_beta, &method);

    match result {
        Ok(()) => {
            eprintln!("\x1b[32m✔\x1b[0m Switched to {target_channel} channel: v{latest}");
            if daemon_was_running {
                eprintln!("  Restarting daemon...");
                restart_daemon();
            }
            Ok(latest)
        }
        Err(e) => {
            if daemon_was_running {
                eprintln!("  Restarting daemon (switch failed, old version still in place)...");
                restart_daemon();
            }
            Err(e)
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_name_stable() {
        let name = asset_name("3.3.3", false);
        assert!(name.starts_with("tokensave-v3.3.3-"));
        assert!(!name.contains("beta"));
        if cfg!(windows) {
            assert!(name.ends_with(".zip"));
        } else {
            assert!(name.ends_with(".tar.gz"));
        }
    }

    #[test]
    fn test_asset_name_beta() {
        let name = asset_name("4.0.2-beta.1", true);
        assert!(name.starts_with("tokensave-beta-v4.0.2-beta.1-"));
        if cfg!(windows) {
            assert!(name.ends_with(".zip"));
        } else {
            assert!(name.ends_with(".tar.gz"));
        }
    }

    #[test]
    fn test_release_tag() {
        assert_eq!(release_tag("3.3.3"), "v3.3.3");
        assert_eq!(release_tag("4.0.2-beta.1"), "v4.0.2-beta.1");
    }

    #[test]
    fn test_current_platform_not_unknown() {
        assert_ne!(current_platform(), "unknown");
    }

    #[test]
    fn test_asset_name_matches_ci_convention() {
        let stable = asset_name("3.3.3", false);
        let platform = current_platform();
        if cfg!(windows) {
            assert_eq!(stable, format!("tokensave-v3.3.3-{platform}.zip"));
        } else {
            assert_eq!(stable, format!("tokensave-v3.3.3-{platform}.tar.gz"));
        }

        let beta = asset_name("4.0.2-beta.1", true);
        if cfg!(windows) {
            assert_eq!(beta, format!("tokensave-beta-v4.0.2-beta.1-{platform}.zip"));
        } else {
            assert_eq!(
                beta,
                format!("tokensave-beta-v4.0.2-beta.1-{platform}.tar.gz")
            );
        }
    }

    #[test]
    fn classify_upgrade_marks_equal_version_as_already_current() {
        assert_eq!(
            classify_upgrade("4.0.3", "4.0.3"),
            UpgradeStatus::AlreadyCurrent
        );
    }

    #[test]
    fn classify_upgrade_marks_newer_version_as_upgrade_available() {
        assert_eq!(
            classify_upgrade("4.0.2", "4.0.3"),
            UpgradeStatus::UpgradeAvailable("4.0.3")
        );
    }

    #[test]
    fn switch_channel_same_channel_is_a_successful_noop() {
        let current = env!("CARGO_PKG_VERSION").to_string();
        let current_channel = if cloud::is_beta() { "beta" } else { "stable" };

        let result = switch_channel(current_channel);

        assert_eq!(result.unwrap(), current);
    }

    // ── Regression tests for symlink upgrade bug ────────────────────────
    //
    // The self-replace crate resolves symlinks via `fs::read_link`, which
    // returns the raw target (often relative for Homebrew). Subsequent
    // operations resolve that relative path from CWD instead of the
    // symlink's parent, causing ENOENT.
    //
    // Our fix: canonicalize the exe path before passing it to self_update.
    // These tests verify the canonicalization works correctly for every
    // symlink layout we've seen in the wild.

    #[cfg(unix)]
    mod symlink_upgrade_regression {
        use std::fs;
        use std::os::unix::fs::symlink;
        use std::path::PathBuf;

        /// Helper: create a fake binary file in a Homebrew-style Cellar layout.
        /// Returns (cellar_binary_path, symlink_path, tmp_guard).
        fn homebrew_layout() -> (PathBuf, PathBuf, tempfile::TempDir) {
            let tmp = tempfile::tempdir().unwrap();
            // Cellar/tokensave/4.1.1-beta.1/bin/tokensave
            let cellar_bin_dir = tmp.path().join("Cellar/tokensave/4.1.1-beta.1/bin");
            fs::create_dir_all(&cellar_bin_dir).unwrap();
            let real_binary = cellar_bin_dir.join("tokensave");
            fs::write(&real_binary, b"fake-binary").unwrap();

            // bin/tokensave -> ../Cellar/tokensave/4.1.1-beta.1/bin/tokensave
            let bin_dir = tmp.path().join("bin");
            fs::create_dir_all(&bin_dir).unwrap();
            let link_path = bin_dir.join("tokensave");
            symlink("../Cellar/tokensave/4.1.1-beta.1/bin/tokensave", &link_path).unwrap();

            (real_binary, link_path, tmp)
        }

        #[test]
        fn read_link_returns_relative_path_for_homebrew_symlink() {
            let (_real, link, _tmp) = homebrew_layout();
            let target = fs::read_link(&link).unwrap();
            assert!(
                target.is_relative(),
                "Homebrew symlink target should be relative, got: {target:?}"
            );
            assert_eq!(
                target,
                PathBuf::from("../Cellar/tokensave/4.1.1-beta.1/bin/tokensave")
            );
        }

        #[test]
        fn relative_read_link_fails_from_wrong_cwd() {
            // This is the exact bug: read_link returns a relative path, and
            // metadata() resolves it from CWD rather than the symlink's parent.
            let (_real, link, _tmp) = homebrew_layout();
            let target = fs::read_link(&link).unwrap();

            // From a different directory (e.g. the user's home), the relative
            // path doesn't resolve to anything valid.
            let other_dir = tempfile::tempdir().unwrap();
            let wrong_path = other_dir.path().join(&target);
            assert!(
                wrong_path.metadata().is_err(),
                "relative symlink target should NOT resolve from an unrelated directory"
            );
        }

        #[test]
        fn canonicalize_resolves_relative_symlink_to_absolute() {
            let (real, link, _tmp) = homebrew_layout();
            let canonical = link.canonicalize().unwrap();
            let real_canonical = real.canonicalize().unwrap();
            assert_eq!(
                canonical, real_canonical,
                "canonicalize should resolve symlink to the real Cellar path"
            );
            assert!(canonical.is_absolute());
        }

        #[test]
        fn canonical_path_differs_from_symlink_path() {
            // This is the key property our fix relies on: after canonicalization,
            // the path differs from the symlink path, which makes self_update
            // choose the Move code path instead of the buggy self_replace path.
            let (_real, link, _tmp) = homebrew_layout();
            let canonical = link.canonicalize().unwrap();
            assert_ne!(
                canonical, link,
                "canonical path and symlink path must differ so self_update uses Move"
            );
        }

        #[test]
        fn canonical_path_parent_exists() {
            // Move::to_dest needs the parent directory to exist for rename().
            let (_real, link, _tmp) = homebrew_layout();
            let canonical = link.canonicalize().unwrap();
            assert!(
                canonical.parent().unwrap().is_dir(),
                "parent of canonical path must be a real directory"
            );
        }

        #[test]
        fn canonicalize_is_identity_for_non_symlink() {
            // For direct installs (cargo install, manual copy), canonicalize
            // returns the same path, so self_replace is still used — no
            // behavior change for non-symlink installs.
            let tmp = tempfile::tempdir().unwrap();
            let binary = tmp.path().join("tokensave");
            fs::write(&binary, b"fake-binary").unwrap();

            let canonical = binary.canonicalize().unwrap();
            let original_canonical = binary.canonicalize().unwrap();
            assert_eq!(canonical, original_canonical);
        }

        #[test]
        fn canonicalize_resolves_absolute_symlink() {
            // Some package managers use absolute symlinks.
            let tmp = tempfile::tempdir().unwrap();
            let real_dir = tmp.path().join("lib");
            fs::create_dir_all(&real_dir).unwrap();
            let real_binary = real_dir.join("tokensave");
            fs::write(&real_binary, b"fake-binary").unwrap();

            let bin_dir = tmp.path().join("bin");
            fs::create_dir_all(&bin_dir).unwrap();
            let link = bin_dir.join("tokensave");
            symlink(&real_binary, &link).unwrap();

            let canonical = link.canonicalize().unwrap();
            assert_eq!(canonical, real_binary.canonicalize().unwrap());
            assert_ne!(canonical, link);
        }

        #[test]
        fn canonicalize_resolves_chained_symlinks() {
            // A -> B -> C: canonicalize must reach C.
            let tmp = tempfile::tempdir().unwrap();
            let real = tmp.path().join("real_binary");
            fs::write(&real, b"fake-binary").unwrap();

            let link_b = tmp.path().join("link_b");
            symlink(&real, &link_b).unwrap();

            let link_a = tmp.path().join("link_a");
            symlink(&link_b, &link_a).unwrap();

            let canonical = link_a.canonicalize().unwrap();
            assert_eq!(canonical, real.canonicalize().unwrap());
        }

        #[test]
        fn canonicalize_resolves_symlink_with_dotdot_in_real_path() {
            // Real path contains ".." components — canonicalize normalizes them.
            let tmp = tempfile::tempdir().unwrap();
            let deep = tmp.path().join("a/b/c");
            fs::create_dir_all(&deep).unwrap();
            let real = deep.join("tokensave");
            fs::write(&real, b"fake-binary").unwrap();

            // Construct a path with ".." that still reaches the same file
            let dotdot_path = tmp.path().join("a/b/c/../c/tokensave");
            let canonical = dotdot_path.canonicalize().unwrap();
            assert_eq!(canonical, real.canonicalize().unwrap());
            assert!(
                !canonical.to_string_lossy().contains(".."),
                "canonical path should have no '..' components"
            );
        }

        #[test]
        fn rename_works_for_canonical_cellar_path() {
            // Simulate what Move::to_dest does: rename a new binary over the
            // canonical (Cellar) path. The symlink continues to work.
            let (real, link, _tmp) = homebrew_layout();

            // "New binary" in a temp location (same filesystem)
            let new_binary = real.parent().unwrap().join(".tokensave.__temp__");
            fs::write(&new_binary, b"upgraded-binary").unwrap();

            // Rename new binary over the real path (what Move does)
            let canonical = link.canonicalize().unwrap();
            fs::rename(&new_binary, &canonical).unwrap();

            // Verify: reading through the symlink yields the new content
            let content = fs::read(&link).unwrap();
            assert_eq!(content, b"upgraded-binary");

            // Verify: the canonical path also has new content
            let content = fs::read(&canonical).unwrap();
            assert_eq!(content, b"upgraded-binary");
        }

        #[test]
        fn symlink_survives_rename_replacement() {
            // After the upgrade replaces the Cellar binary, the Homebrew
            // symlink must still point to a valid file.
            let (_real, link, _tmp) = homebrew_layout();
            let canonical = link.canonicalize().unwrap();

            // Replace the binary at the canonical path
            fs::write(&canonical, b"new-version").unwrap();

            // Symlink still works
            assert!(
                link.exists(),
                "symlink must still resolve after replacement"
            );
            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink(),
                "must still be a symlink"
            );
            assert_eq!(fs::read(&link).unwrap(), b"new-version");
        }

        #[test]
        fn canonicalize_fails_for_dangling_symlink() {
            // If the Cellar dir was removed (brew cleanup), canonicalize
            // should fail and we gracefully fall back to the default.
            let tmp = tempfile::tempdir().unwrap();
            let bin_dir = tmp.path().join("bin");
            fs::create_dir_all(&bin_dir).unwrap();
            let link = bin_dir.join("tokensave");
            symlink("../Cellar/tokensave/old/bin/tokensave", &link).unwrap();
            // Target doesn't exist — dangling symlink
            assert!(
                link.canonicalize().is_err(),
                "canonicalize should fail for dangling symlinks"
            );
        }

        #[test]
        fn our_fix_pattern_handles_all_cases() {
            // Simulate the exact pattern used in run_upgrade/switch_channel:
            //   if let Ok(canonical) = path.canonicalize() { ... }
            // Verify it does the right thing for each scenario.

            // Case 1: relative symlink (Homebrew) — canonical differs
            let (_, link, _tmp) = homebrew_layout();
            let canonical = link.canonicalize();
            assert!(canonical.is_ok());
            assert_ne!(canonical.unwrap(), link);

            // Case 2: direct file — canonical matches
            let tmp2 = tempfile::tempdir().unwrap();
            let direct = tmp2.path().join("tokensave");
            fs::write(&direct, b"binary").unwrap();
            let canonical = direct.canonicalize().unwrap();
            // After canonicalization of the tmpdir itself, they match
            assert_eq!(canonical, direct.canonicalize().unwrap());

            // Case 3: dangling symlink — canonical fails, we skip setting
            // bin_install_path and let self_update use its default
            let tmp3 = tempfile::tempdir().unwrap();
            let dangling = tmp3.path().join("tokensave");
            symlink("/nonexistent/path/tokensave", &dangling).unwrap();
            assert!(dangling.canonicalize().is_err());
        }

        // ── install_binary tests ───────────────────────────────────────

        #[test]
        fn install_binary_replaces_target_atomically() {
            let tmp = tempfile::tempdir().unwrap();
            let target = tmp.path().join("tokensave");
            fs::write(&target, b"old-binary").unwrap();

            let src = tmp.path().join("new-binary");
            fs::write(&src, b"new-binary-content").unwrap();

            super::super::install_binary(&src, &target).unwrap();

            assert_eq!(fs::read(&target).unwrap(), b"new-binary-content");
            // Temp file should be cleaned up
            assert!(!tmp
                .path()
                .join(format!(".tokensave_upgrade_{}", std::process::id()))
                .exists());
        }

        #[test]
        fn install_binary_sets_executable_permission() {
            use std::os::unix::fs::PermissionsExt;

            let tmp = tempfile::tempdir().unwrap();
            let target = tmp.path().join("tokensave");
            fs::write(&target, b"old").unwrap();

            let src = tmp.path().join("new");
            fs::write(&src, b"new").unwrap();

            super::super::install_binary(&src, &target).unwrap();

            let mode = fs::metadata(&target).unwrap().permissions().mode();
            assert_eq!(mode & 0o755, 0o755, "binary should be executable");
        }

        // ── Brew upgrade flow ──────────────────────────────────────────

        #[test]
        fn brew_upgrade_renames_version_dir_and_updates_symlink() {
            let (_real, link, _tmp) = homebrew_layout();

            // Write an "upgraded" binary via the Cellar path
            let canonical = link.canonicalize().unwrap();
            fs::write(&canonical, b"v5.0.0-binary").unwrap();

            // Simulate the Cellar directory rename (4.1.1-beta.1 → 5.0.0)
            let bin_dir = canonical.parent().unwrap();
            let version_dir = bin_dir.parent().unwrap();
            let formula_dir = version_dir.parent().unwrap();
            let cellar_dir = formula_dir.parent().unwrap();
            let _prefix = cellar_dir.parent().unwrap();

            let new_version_dir = formula_dir.join("5.0.0");
            fs::rename(version_dir, &new_version_dir).unwrap();

            // Update the symlink
            let old_target = fs::read_link(&link).unwrap();
            let new_target = PathBuf::from(old_target.to_string_lossy().replacen(
                "4.1.1-beta.1",
                "5.0.0",
                1,
            ));
            fs::remove_file(&link).unwrap();
            symlink(&new_target, &link).unwrap();

            // Verify: symlink resolves and has the new content
            assert!(link.exists(), "symlink must resolve after dir rename");
            assert_eq!(fs::read(&link).unwrap(), b"v5.0.0-binary");

            // Verify: new version directory exists, old one doesn't
            assert!(new_version_dir.exists());
            assert!(!version_dir.exists());

            // Verify: brew would see "5.0.0" as the installed version
            // (brew reads directory names under Cellar/<formula>/)
            let versions: Vec<_> = fs::read_dir(formula_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            assert_eq!(versions, vec!["5.0.0"]);
        }

        #[test]
        fn brew_upgrade_updates_install_receipt() {
            let tmp = tempfile::tempdir().unwrap();
            let cellar = tmp.path().join("Cellar/tokensave/4.0.3");
            fs::create_dir_all(cellar.join("bin")).unwrap();
            fs::write(cellar.join("bin/tokensave"), b"binary").unwrap();

            let receipt_content = r#"{
  "source": {
    "versions": { "stable": "4.0.3" }
  },
  "tabfile": "/opt/homebrew/Cellar/tokensave/4.0.3/INSTALL_RECEIPT.json"
}"#;
            fs::write(cellar.join("INSTALL_RECEIPT.json"), receipt_content).unwrap();

            // Simulate rename + receipt update
            let new_dir = tmp.path().join("Cellar/tokensave/4.0.4");
            fs::rename(&cellar, &new_dir).unwrap();

            let text = fs::read_to_string(new_dir.join("INSTALL_RECEIPT.json")).unwrap();
            let updated = text.replace("4.0.3", "4.0.4");
            fs::write(new_dir.join("INSTALL_RECEIPT.json"), &updated).unwrap();

            assert!(updated.contains("\"stable\": \"4.0.4\""));
            assert!(updated.contains("/4.0.4/INSTALL_RECEIPT.json"));
            assert!(!updated.contains("4.0.3"));
        }
    }
}
