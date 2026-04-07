//! Self-update for the tokensave binary.
//!
//! Downloads the latest release asset directly from GitHub, extracts the
//! binary, and replaces the running executable using self_replace.
//! Beta and stable are separate channels — a beta build only sees beta
//! releases and vice versa. The daemon is stopped before the binary is
//! replaced and restarted afterwards if it was running.

use std::path::Path;

use crate::cloud;
use crate::daemon;
use crate::errors::{Result, TokenSaveError};

const GITHUB_REPO: &str = "aovestdipaperino/tokensave";

/// Archive naming convention per platform.
/// Stable: `tokensave-v{version}-{platform}.{ext}`
/// Beta:   `tokensave-beta-v{version}-{platform}.{ext}`
fn asset_name(version: &str, is_beta: bool) -> String {
    let prefix = if is_beta { "tokensave-beta" } else { "tokensave" };
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
        .timeout_global(Some(std::time::Duration::from_secs(300)))
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

        if Path::new(file.name())
            .file_name()
            .and_then(|n| n.to_str())
            == Some(bin_name)
        {
            let mut out =
                std::fs::File::create(dest).map_err(io_err("create temp file failed"))?;
            std::io::copy(&mut file, &mut out).map_err(io_err("extract failed"))?;
            return Ok(());
        }
    }

    Err(TokenSaveError::Config {
        message: format!("binary '{bin_name}' not found in zip"),
    })
}

/// Replaces the running binary with `new_exe`, then removes the temp file.
fn replace_binary(new_exe: &Path) -> Result<()> {
    let result = self_replace::self_replace(new_exe).map_err(|e| TokenSaveError::Config {
        message: format!(
            "binary replacement failed: {e}\n  \
             The old version is still in place.\n  \
             To upgrade manually: https://github.com/{GITHUB_REPO}/releases/latest"
        ),
    });
    let _ = std::fs::remove_file(new_exe);
    result
}

/// Downloads, extracts, and installs the binary for `version`/`is_beta`.
fn perform_upgrade(version: &str, is_beta: bool) -> Result<()> {
    let tag = release_tag(version);
    let expected = asset_name(version, is_beta);
    let bin_name = if cfg!(windows) { "tokensave.exe" } else { "tokensave" };

    eprintln!("  Asset: {expected}");

    let url = fetch_asset_url(&tag, &expected)?;
    let tmp = download_and_extract(&url, bin_name)?;

    eprint!("  Replacing binary...");
    replace_binary(&tmp)?;
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
        Err(e) => eprintln!(
            "  \x1b[33mwarning:\x1b[0m failed to restart daemon: {e}"
        ),
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

    eprintln!("Current version: v{current} ({channel} channel)");
    eprintln!("Checking for updates...");

    let latest = cloud::fetch_latest_version().ok_or_else(|| TokenSaveError::Config {
        message: "failed to check for updates — could not reach GitHub".to_string(),
    })?;

    if !cloud::is_newer_version(current, &latest) {
        eprintln!("\x1b[32m✔\x1b[0m Already up to date (v{current}).");
        return Err(TokenSaveError::Config {
            message: format!("already at latest version v{current}"),
        });
    }

    eprintln!("Upgrading v{current} → v{latest}...");

    let daemon_was_running = daemon::running_daemon_pid().is_some();
    if daemon_was_running {
        eprintln!("  Stopping daemon...");
        daemon::stop().ok();
    }

    let result = perform_upgrade(&latest, is_beta);

    match result {
        Ok(()) => {
            eprintln!("\x1b[32m✔\x1b[0m Successfully upgraded to v{latest}!");
            if daemon_was_running {
                eprintln!("  Restarting daemon...");
                restart_daemon();
            }
            Ok(latest)
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

    let target_is_beta = match target_channel {
        "beta" => true,
        "stable" => false,
        other => {
            return Err(TokenSaveError::Config {
                message: format!("unknown channel '{other}'. Valid channels: stable, beta"),
            });
        }
    };

    if target_is_beta == current_is_beta {
        eprintln!("Already on the {current_channel} channel (v{current}).");
        eprintln!("Run `tokensave upgrade` to check for updates within this channel.");
        return Err(TokenSaveError::Config {
            message: format!("already on {current_channel} channel"),
        });
    }

    eprintln!("Switching from {current_channel} to {target_channel}...");

    let latest = if target_is_beta {
        cloud::fetch_latest_beta_version()
    } else {
        cloud::fetch_latest_stable_version()
    }
    .ok_or_else(|| TokenSaveError::Config {
        message: format!(
            "failed to find latest {target_channel} release — could not reach GitHub"
        ),
    })?;

    eprintln!("  Target: v{latest}");

    let daemon_was_running = daemon::running_daemon_pid().is_some();
    if daemon_was_running {
        eprintln!("  Stopping daemon...");
        daemon::stop().ok();
    }

    let result = perform_upgrade(&latest, target_is_beta);

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
                eprintln!(
                    "  Restarting daemon (switch failed, old version still in place)..."
                );
                restart_daemon();
            }
            Err(e)
        }
    }
}

#[cfg(test)]
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
            assert_eq!(beta, format!("tokensave-beta-v4.0.2-beta.1-{platform}.tar.gz"));
        }
    }
}
