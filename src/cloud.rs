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
fn agent_with_timeout(timeout: Duration) -> ureq::Agent {
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
    let mut resp = match agent
        .get(&format!("{WORKER_URL}/countries"))
        .call()
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let parsed: CountriesResponse = match resp.body_mut().read_json() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    parsed.flags
}

/// Response from GitHub releases API (only the fields we need).
#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
}

/// Fetches the latest release version from GitHub.
/// For beta builds, fetches the latest prerelease; for stable builds,
/// fetches the latest stable release. This ensures each channel only
/// sees updates from its own channel.
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
    // Find the first prerelease in the list (sorted newest first by GitHub)
    releases
        .into_iter()
        .find(|r| r.prerelease)
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
                (None, None) => false,
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
