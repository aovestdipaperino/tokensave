//! HTTP client for the worldwide token counter Cloudflare Worker and
//! GitHub release version checking.
//!
//! All operations are best-effort with timeouts. Failures are silently
//! ignored and never block the CLI.

use std::time::Duration;

/// The Cloudflare Worker endpoint URL.
const WORKER_URL: &str = "https://tokensave-counter.enzinol.workers.dev";

/// GitHub API endpoint for the latest release.
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/aovestdipaperino/tokensave/releases/latest";

/// Timeout for flush (upload) requests.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// Timeout for fetching the worldwide total (used in status).
const FETCH_TIMEOUT: Duration = Duration::from_secs(1);

/// Response from the worker's POST /increment and GET /total endpoints.
#[derive(serde::Deserialize)]
struct WorkerResponse {
    total: u64,
}

/// Uploads pending tokens to the worldwide counter.
/// Returns the new worldwide total on success, or None on any failure.
pub fn flush_pending(amount: u64) -> Option<u64> {
    if amount == 0 {
        return None;
    }
    let body = serde_json::json!({ "amount": amount });
    let agent = ureq::AgentBuilder::new().timeout(FLUSH_TIMEOUT).build();
    let resp = agent
        .post(&format!("{WORKER_URL}/increment"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .ok()?;
    let parsed: WorkerResponse = resp.into_json().ok()?;
    Some(parsed.total)
}

/// Fetches the current worldwide total from the worker.
/// Returns None on timeout, network error, or parse failure.
pub fn fetch_worldwide_total() -> Option<u64> {
    let agent = ureq::AgentBuilder::new().timeout(FETCH_TIMEOUT).build();
    let resp = agent
        .get(&format!("{WORKER_URL}/total"))
        .call()
        .ok()?;
    let parsed: WorkerResponse = resp.into_json().ok()?;
    Some(parsed.total)
}

/// Response from GitHub releases API (only the fields we need).
#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

/// Fetches the latest release version from GitHub.
/// Returns the version string (without leading 'v') or None on failure.
pub fn fetch_latest_version() -> Option<String> {
    let agent = ureq::AgentBuilder::new().timeout(FETCH_TIMEOUT).build();
    let resp = agent
        .get(GITHUB_RELEASES_URL)
        .set("User-Agent", "tokensave")
        .call()
        .ok()?;
    let release: GitHubRelease = resp.into_json().ok()?;
    Some(release.tag_name.trim_start_matches('v').to_string())
}

/// Returns true if `latest` is strictly newer than `current` using semver comparison.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let mut parts = v.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        Some((major, minor, patch))
    };
    match (parse(current), parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// How tokensave was installed, detected from the binary path.
pub enum InstallMethod {
    Cargo,
    Brew,
    Unknown,
}

/// Detects how tokensave was installed by inspecting the binary path.
pub fn detect_install_method() -> InstallMethod {
    let Ok(exe) = std::env::current_exe() else {
        return InstallMethod::Unknown;
    };
    let path = exe.to_string_lossy();
    if path.contains(".cargo/bin") {
        InstallMethod::Cargo
    } else if path.contains("/homebrew/") || path.contains("/Cellar/") {
        InstallMethod::Brew
    } else {
        InstallMethod::Unknown
    }
}

/// Returns the upgrade command string for the detected install method.
pub fn upgrade_command(method: &InstallMethod) -> &'static str {
    match method {
        InstallMethod::Cargo => "cargo install tokensave",
        InstallMethod::Brew => "brew upgrade tokensave",
        InstallMethod::Unknown => "cargo install tokensave",
    }
}
