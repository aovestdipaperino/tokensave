//! HTTP client for the worldwide token counter Cloudflare Worker and
//! GitHub release version checking.
//!
//! All operations are best-effort with timeouts. Failures are silently
//! ignored and never block the CLI.

use std::time::Duration;

/// The Cloudflare Worker endpoint URL.
const WORKER_URL: &str = "https://tokensave-counter.enzinol.workers.dev";

/// GitHub API endpoint for the latest stable release.
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/aovestdipaperino/tokensave/releases/latest";

/// GitHub API endpoint for listing releases (used to find latest beta).
const GITHUB_RELEASES_LIST_URL: &str =
    "https://api.github.com/repos/aovestdipaperino/tokensave/releases?per_page=10";

/// Timeout for flush (upload) requests.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// Timeout for fetching the worldwide total (used in status).
const FETCH_TIMEOUT: Duration = Duration::from_secs(1);

/// Response from the worker's POST /increment and GET /total endpoints.
#[derive(serde::Deserialize)]
struct WorkerResponse {
    total: u64,
}

/// Creates a ureq agent with the given timeout.
pub fn agent_with_timeout(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// Uploads pending tokens to the worldwide counter.
/// Returns the new worldwide total on success, or None on any failure.
pub fn flush_pending(amount: u64) -> Option<u64> {
    if amount == 0 {
        return None;
    }
    let body = serde_json::json!({ "amount": amount });
    let agent = agent_with_timeout(FLUSH_TIMEOUT);
    let parsed: WorkerResponse = agent
        .post(&format!("{WORKER_URL}/increment"))
        .send_json(&body)
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    Some(parsed.total)
}

/// Fetches the current worldwide total from the worker.
/// Returns None on timeout, network error, or parse failure.
pub fn fetch_worldwide_total() -> Option<u64> {
    let agent = agent_with_timeout(FETCH_TIMEOUT);
    let parsed: WorkerResponse = agent
        .get(&format!("{WORKER_URL}/total"))
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    Some(parsed.total)
}

/// Response from the worker's GET /countries endpoint.
#[derive(serde::Deserialize)]
struct CountriesResponse {
    flags: Vec<String>,
}

/// Fetches country flags from the worldwide counter.
/// Returns a list of emoji flags, or an empty vec on failure.
pub fn fetch_country_flags() -> Vec<String> {
    let agent = agent_with_timeout(Duration::from_millis(500));
    let Ok(mut resp) = agent.get(&format!("{WORKER_URL}/countries")).call() else {
        return Vec::new();
    };
    let Ok(parsed): Result<CountriesResponse, _> = resp.body_mut().read_json() else {
        return Vec::new();
    };
    parsed.flags
}

/// Response from GitHub releases API (only the fields we need).
#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
    name: String,
}

/// Returns the platform slug matching the CI release matrix. Must stay in
/// sync with the `matrix.name` field in `.github/workflows/release.yml`
/// and `release-beta.yml`.
pub(crate) fn current_platform() -> &'static str {
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

/// Archive naming convention per platform. Must stay in sync with the
/// `tar czf` / `Compress-Archive` invocations in `.github/workflows/release.yml`
/// and `release-beta.yml`:
///
/// - Stable: `tokensave-v{version}-{platform}.{ext}`
/// - Beta:   `tokensave-beta-v{version}-{platform}.{ext}`
pub(crate) fn asset_name(version: &str, is_beta: bool) -> String {
    let prefix = if is_beta {
        "tokensave-beta"
    } else {
        "tokensave"
    };
    let platform = current_platform();
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("{prefix}-v{version}-{platform}.{ext}")
}

/// True when the release lists an asset matching the current platform.
/// Filters out releases whose CI build hasn't finished uploading binaries
/// for the current target — otherwise we'd announce a version the user
/// cannot actually install.
fn release_has_current_platform_asset(release: &GitHubRelease) -> bool {
    let version = release.tag_name.trim_start_matches('v');
    let expected = asset_name(version, release.prerelease);
    release.assets.iter().any(|a| a.name == expected)
}

/// Fetches the latest release version from GitHub.
/// For beta builds, fetches the latest prerelease; for stable builds,
/// fetches the latest stable release. This ensures each channel only
/// sees updates from its own channel. Releases whose CI hasn't yet
/// uploaded the current-platform binary are skipped — see
/// `release_has_current_platform_asset`.
pub fn fetch_latest_version() -> Option<String> {
    if is_beta() {
        fetch_latest_beta_version()
    } else {
        fetch_latest_stable_version()
    }
}

/// Fetches the latest stable release version from GitHub.
pub fn fetch_latest_stable_version() -> Option<String> {
    let agent = agent_with_timeout(FETCH_TIMEOUT);
    let release: GitHubRelease = agent
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", "tokensave")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    if !release_has_current_platform_asset(&release) {
        return None;
    }
    Some(release.tag_name.trim_start_matches('v').to_string())
}

/// Fetches the latest prerelease version from GitHub.
pub fn fetch_latest_beta_version() -> Option<String> {
    let agent = agent_with_timeout(FETCH_TIMEOUT);
    let releases: Vec<GitHubRelease> = agent
        .get(GITHUB_RELEASES_LIST_URL)
        .header("User-Agent", "tokensave")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    // First prerelease that has the current platform's asset already
    // uploaded. GitHub returns the list newest-first, so the first match
    // is the latest installable beta. Releases whose CI is still in
    // progress are skipped — they will be picked up on the next check.
    releases
        .into_iter()
        .find(|r| r.prerelease && release_has_current_platform_asset(r))
        .map(|r| r.tag_name.trim_start_matches('v').to_string())
}

/// Returns true if the current build is a beta/prerelease version.
pub fn is_beta() -> bool {
    env!("CARGO_PKG_VERSION").contains('-')
}

/// Returns true if `latest` is strictly newer than `current` using semver comparison.
/// Handles pre-release suffixes (e.g. "2.5.0-beta.1") by stripping them for the
/// base version comparison, then comparing pre-release tags lexicographically.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    /// Parses a version string into (major, minor, patch, pre-release).
    fn parse(v: &str) -> Option<(u64, u64, u64, Option<&str>)> {
        let (base, pre) = match v.split_once('-') {
            Some((b, p)) => (b, Some(p)),
            None => (v, None),
        };
        let mut parts = base.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        Some((major, minor, patch, pre))
    }

    match (parse(current), parse(latest)) {
        (Some((cm, cn, cp, cpre)), Some((lm, ln, lp, lpre))) => {
            // Beta and stable are separate channels — never suggest cross-channel updates.
            if cpre.is_some() != lpre.is_some() {
                return false;
            }
            let c_base = (cm, cn, cp);
            let l_base = (lm, ln, lp);
            if l_base != c_base {
                return l_base > c_base;
            }
            // Same base version, same channel
            match (cpre, lpre) {
                (Some(a), Some(b)) => b > a,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Returns true if `latest` is a newer version than `current` AND the
/// difference is at least a minor version bump (patch-only bumps return false).
///
/// Used by the CLI version warning to avoid nagging on patch releases.
pub fn is_newer_minor_version(current: &str, latest: &str) -> bool {
    fn parse(v: &str) -> Option<(u64, u64)> {
        let base = v.split_once('-').map_or(v, |(b, _)| b);
        let mut parts = base.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        Some((major, minor))
    }

    is_newer_version(current, latest)
        && match (parse(current), parse(latest)) {
            (Some(c), Some(l)) => l > c,
            _ => true,
        }
}

/// How tokensave was installed, detected from the binary path.
pub enum InstallMethod {
    Cargo,
    Brew,
    Scoop,
    Unknown,
}

/// Detects how tokensave was installed by inspecting the binary path.
pub fn detect_install_method() -> InstallMethod {
    let Ok(exe) = std::env::current_exe() else {
        return InstallMethod::Unknown;
    };
    let path = exe.to_string_lossy();
    if path.contains(".cargo/bin") || path.contains(".cargo\\bin") {
        InstallMethod::Cargo
    } else if path.contains("/homebrew/") || path.contains("/Cellar/") {
        InstallMethod::Brew
    } else if path.contains("\\scoop\\") || path.contains("/scoop/") {
        InstallMethod::Scoop
    } else {
        InstallMethod::Unknown
    }
}

/// Returns the upgrade command string.
///
/// Always suggests `tokensave upgrade` which handles all install methods
/// and channels automatically.
pub fn upgrade_command(_method: &InstallMethod) -> &'static str {
    "tokensave upgrade"
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, asset_names: &[&str]) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.to_string(),
            prerelease,
            assets: asset_names
                .iter()
                .map(|n| GitHubAsset {
                    name: (*n).to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn skips_release_with_no_assets() {
        // A release that was just created — CI hasn't started uploading yet.
        let r = release("v9.9.9", false, &[]);
        assert!(!release_has_current_platform_asset(&r));
    }

    #[test]
    fn skips_release_missing_current_platform_asset() {
        // Other platforms uploaded but ours hasn't yet (e.g. the macOS leg
        // of the matrix is still running). Detection should treat this as
        // "no upgrade for me" so the user isn't told about a version they
        // cannot install.
        let r = release(
            "v9.9.9",
            false,
            &[
                "tokensave-v9.9.9-some-other-platform.tar.gz",
                "tokensave-v9.9.9-yet-another-platform.tar.gz",
            ],
        );
        assert!(!release_has_current_platform_asset(&r));
    }

    #[test]
    fn accepts_release_with_matching_asset() {
        let expected = asset_name("9.9.9", false);
        let r = release("v9.9.9", false, &[&expected]);
        assert!(release_has_current_platform_asset(&r));
    }

    #[test]
    fn accepts_beta_release_with_matching_beta_asset() {
        let expected = asset_name("9.9.9-beta.1", true);
        let r = release("v9.9.9-beta.1", true, &[&expected]);
        assert!(release_has_current_platform_asset(&r));
    }

    #[test]
    fn rejects_stable_named_asset_on_beta_release() {
        // If someone uploads a `tokensave-v...` asset to a prerelease, the
        // filter should still reject — the naming convention says beta
        // releases carry `tokensave-beta-v...` assets.
        let stable_name = asset_name("9.9.9-beta.1", false);
        let r = release("v9.9.9-beta.1", true, &[&stable_name]);
        assert!(!release_has_current_platform_asset(&r));
    }
}
