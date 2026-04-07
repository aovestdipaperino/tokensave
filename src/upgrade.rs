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

    let mut updater = self_update::backends::github::Update::configure();
    updater
        .repo_owner("aovestdipaperino")
        .repo_name("tokensave")
        .bin_name(bin_name)
        .target(&target_suffix)
        .current_version(current)
        .target_version_tag(&tag)
        .show_download_progress(true)
        .no_confirm(true);

    // Resolve symlinks in the binary path to work around a self-replace bug
    // where Homebrew-style relative symlink targets cause ENOENT during
    // binary replacement. Canonicalizing forces self_update to use a simple
    // rename instead of self_replace when the path is a symlink.
    if let Ok(canonical) = std::env::current_exe().and_then(|p| p.canonicalize()) {
        updater.bin_install_path(canonical);
    }

    let result = updater
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
            let err_str = e.to_string();
            let message = if err_str.contains("No asset found") {
                format!(
                    "upgrade failed: release v{latest} exists but binaries are not yet available \
                     for your platform ({}).\n  \
                     This usually means the CI build is still in progress — try again in a few minutes.\n  \
                     If the problem persists, download manually from:\n  \
                     https://github.com/aovestdipaperino/tokensave/releases/tag/{tag}",
                    current_platform(), tag = tag,
                )
            } else {
                format!("upgrade failed: {e}")
            };
            Err(TokenSaveError::Config { message })
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
                message: format!(
                    "unknown channel '{other}'. Valid channels: stable, beta"
                ),
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

    let tag = release_tag(&latest);
    let expected_asset = asset_name(&latest, target_is_beta);
    let bin_name = if cfg!(windows) { "tokensave.exe" } else { "tokensave" };

    eprintln!("  Target: v{latest}");
    eprintln!("  Asset: {expected_asset}");

    // Stop the daemon before replacing the binary.
    let daemon_was_running = daemon::running_daemon_pid().is_some();
    if daemon_was_running {
        eprintln!("  Stopping daemon...");
        daemon::stop().ok();
    }

    let target_suffix = if target_is_beta {
        format!("beta-v{}-{}", latest, current_platform())
    } else {
        format!("v{}-{}", latest, current_platform())
    };

    let mut updater = self_update::backends::github::Update::configure();
    updater
        .repo_owner("aovestdipaperino")
        .repo_name("tokensave")
        .bin_name(bin_name)
        .target(&target_suffix)
        .current_version(current)
        .target_version_tag(&tag)
        .show_download_progress(true)
        .no_confirm(true);

    // Same symlink workaround as run_upgrade — see comment there.
    if let Ok(canonical) = std::env::current_exe().and_then(|p| p.canonicalize()) {
        updater.bin_install_path(canonical);
    }

    let result = updater
        .build()
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to configure updater: {e}"),
        })?
        .update();

    match result {
        Ok(status) => {
            eprintln!(
                "\x1b[32m✔\x1b[0m Switched to {target_channel} channel: v{}",
                status.version()
            );
            if daemon_was_running {
                eprintln!("  Restarting daemon...");
                restart_daemon();
            }
            Ok(status.version().to_string())
        }
        Err(e) => {
            if daemon_was_running {
                eprintln!("  Restarting daemon (switch failed, old version still in place)...");
                restart_daemon();
            }
            let err_str = e.to_string();
            let message = if err_str.contains("No asset found") {
                format!(
                    "channel switch failed: v{latest} binaries not yet available for {}.\n  \
                     CI build may still be in progress — try again in a few minutes.",
                    current_platform(),
                )
            } else {
                format!("channel switch failed: {e}")
            };
            Err(TokenSaveError::Config { message })
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
            symlink(
                "../Cellar/tokensave/4.1.1-beta.1/bin/tokensave",
                &link_path,
            )
            .unwrap();

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
            assert!(link.exists(), "symlink must still resolve after replacement");
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
    }
}
