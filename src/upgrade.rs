//! Self-update for the tokensave binary.
//!
//! Downloads the latest release from GitHub and replaces the running binary.
//! Beta and stable are separate channels — a beta build only sees beta
//! releases and vice versa. The daemon is stopped before the binary is
//! replaced and restarted afterwards if it was running.

use crate::cloud;
use crate::daemon;
use crate::errors::{Result, TokenSaveError};

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
/// Stable tags: `v3.3.3`, beta tags: `v4.0.2-beta.1`.
fn release_tag(version: &str) -> String {
    format!("v{version}")
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

    let tag = release_tag(&latest);
    let expected_asset = asset_name(&latest, is_beta);
    let bin_name = if cfg!(windows) { "tokensave.exe" } else { "tokensave" };

    eprintln!("Upgrading v{current} → v{latest}...");
    eprintln!("  Asset: {expected_asset}");

    // Stop the daemon before replacing the binary (if running).
    let daemon_was_running = daemon::running_daemon_pid().is_some();
    if daemon_was_running {
        eprintln!("  Stopping daemon...");
        daemon::stop().ok(); // best-effort
    }

    // The `target` in self_update is matched against the asset filename.
    // Our assets are named `tokensave-v3.3.3-aarch64-macos.tar.gz`, so
    // we set target to the platform portion that self_update appends:
    // `{bin_name}-{target}.{ext}` where target = what we pass here.
    //
    // self_update constructs: `{bin_name}-{target}.tar.gz` (or .zip)
    // We need it to match our asset: `tokensave-v{version}-{platform}.tar.gz`
    // So target should be: `v{version}-{platform}` for stable,
    //                       `beta-v{version}-{platform}` for beta
    // (because bin_name already provides the `tokensave-` prefix).
    let target_suffix = if is_beta {
        format!("beta-v{}-{}", latest, current_platform())
    } else {
        format!("v{}-{}", latest, current_platform())
    };

    let result = self_update::backends::github::Update::configure()
        .repo_owner("aovestdipaperino")
        .repo_name("tokensave")
        .bin_name(bin_name)
        .target(&target_suffix)
        .current_version(current)
        .target_version_tag(&tag)
        .show_download_progress(true)
        .no_confirm(true)
        .build()
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to configure updater: {e}"),
        })?
        .update();

    match result {
        Ok(status) => {
            eprintln!(
                "\x1b[32m✔\x1b[0m Successfully upgraded to v{}!",
                status.version()
            );

            // Restart the daemon if it was running before the upgrade.
            if daemon_was_running {
                eprintln!("  Restarting daemon...");
                // The new binary is now in place; spawning `tokensave daemon`
                // will use it. We use a detached process rather than the
                // in-process daemon::run() since the current binary is the old
                // version.
                restart_daemon();
            }

            Ok(status.version().to_string())
        }
        Err(e) => {
            // Restart daemon even on failure — the old binary is still in place.
            if daemon_was_running {
                eprintln!("  Restarting daemon (upgrade failed, old version still in place)...");
                restart_daemon();
            }
            Err(TokenSaveError::Config {
                message: format!("upgrade failed: {e}"),
            })
        }
    }
}

/// Restart the daemon by spawning a detached `tokensave daemon` process.
///
/// Uses the current executable path (which is now the new binary after a
/// successful upgrade) to launch the daemon in the background.
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
        // Stable: tokensave-v{version}-{platform}.tar.gz
        let stable = asset_name("3.3.3", false);
        let platform = current_platform();
        if cfg!(windows) {
            assert_eq!(stable, format!("tokensave-v3.3.3-{platform}.zip"));
        } else {
            assert_eq!(stable, format!("tokensave-v3.3.3-{platform}.tar.gz"));
        }

        // Beta: tokensave-beta-v{version}-{platform}.tar.gz
        let beta = asset_name("4.0.2-beta.1", true);
        if cfg!(windows) {
            assert_eq!(beta, format!("tokensave-beta-v4.0.2-beta.1-{platform}.zip"));
        } else {
            assert_eq!(beta, format!("tokensave-beta-v4.0.2-beta.1-{platform}.tar.gz"));
        }
    }
}
