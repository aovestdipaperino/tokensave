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
///
/// Root certificates come from the OS trust store (via `RootCerts::PlatformVerifier`)
/// rather than ureq's bundled Mozilla roots, so a corporate TLS-intercepting proxy
/// whose root CA is installed in the OS store (e.g. Cato) doesn't break every
/// HTTPS call tokensave makes. TLS verification itself is unaffected — only the
/// trust anchor changes.
pub fn agent_with_timeout(timeout: Duration) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};

    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

/// How long a successful upload holds off the next one: once a day.
///
/// The counter is a single global sum, so nothing about it needs a finer
/// grain than this — an intermediate upload changes the number it reports for
/// a few hours and then is indistinguishable from having waited. What the old
/// 30-second cadence did buy was one network request per command on an active
/// machine, from every path that touches the counter.
pub const UPLOAD_INTERVAL_SECS: i64 = 24 * 60 * 60;

/// How long a *failed* attempt holds off the next one.
///
/// Deliberately much shorter than [`UPLOAD_INTERVAL_SECS`]: a failure leaves
/// `last_upload_at` untouched, so the daily gate stays open and this is the
/// only thing standing between a broken network and a request per command.
pub const FAILED_ATTEMPT_COOLDOWN_SECS: i64 = 60;

/// Whether an upload is due: uploads are enabled, something is pending, a day
/// has passed since the last success, and the last attempt did not just fail.
///
/// Every path that uploads goes through this, so the cadence is one decision
/// in one place rather than a staleness check repeated per call site with its
/// own threshold (and, at two of them, none at all).
pub fn upload_is_due(config: &crate::user_config::UserConfig, now: i64) -> bool {
    if config.pending_upload == 0 || !config.upload_enabled {
        return false;
    }
    // A failed attempt is recorded past the last success; back off briefly so a
    // broken network does not mean a request per command.
    if config.last_flush_attempt_at > config.last_upload_at
        && now - config.last_flush_attempt_at < FAILED_ATTEMPT_COOLDOWN_SECS
    {
        return false;
    }
    // A machine that has never uploaded has `last_upload_at == 0`, which is due.
    now - config.last_upload_at >= UPLOAD_INTERVAL_SECS
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

/// Whether the passive "update available" notice should run.
///
/// On by default. Set `TOKENSAVE_UPDATE_CHECK` to a falsey value
/// (`off`, `false`, `0`, `no`, `disable`, `disabled`) to silence the notice
/// emitted by `init`, `sync`, `status`, and the MCP server. Explicit
/// `tokensave upgrade` and `tokensave doctor` do not consult this and always
/// check.
pub fn update_check_enabled() -> bool {
    update_check_enabled_from(std::env::var("TOKENSAVE_UPDATE_CHECK").ok().as_deref())
}

fn update_check_enabled_from(value: Option<&str>) -> bool {
    match value {
        Some(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off" | "disable" | "disabled"
        ),
        None => true,
    }
}

/// Passive counterpart of [`fetch_latest_version`] for the notice paths.
/// Returns `None` without touching the network when the update check is
/// disabled via `TOKENSAVE_UPDATE_CHECK`.
pub fn fetch_latest_version_passive() -> Option<String> {
    if !update_check_enabled() {
        return None;
    }
    fetch_latest_version()
}

/// Why a version check produced no installable version.
///
/// These have different remedies and must not share a message: collapsing
/// them into a single `None` is what made a release with no asset for the
/// running platform report itself as an unreachable network (#513).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionCheckError {
    /// The releases API could not be reached, or its answer could not be read.
    Unreachable {
        /// What the HTTP layer actually reported — a 404 and a connection
        /// timeout are both worth telling apart.
        detail: String,
    },
    /// GitHub answered, but the release has no asset for this platform.
    NoAssetForPlatform {
        /// Version of the release that is missing the asset, without the `v`.
        version: String,
        /// Platform slug the running binary needs.
        platform: String,
        /// Asset name that was looked for and not found.
        expected: String,
        /// Asset names the release does publish.
        available: Vec<String>,
    },
    /// GitHub answered, but the channel has no release at all.
    NoRelease {
        /// Channel that is empty, as the user names it.
        channel: &'static str,
    },
}

impl std::fmt::Display for VersionCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable { detail } => write!(f, "could not reach GitHub ({detail})"),
            Self::NoAssetForPlatform {
                version,
                platform,
                expected,
                available,
            } => {
                write!(
                    f,
                    "v{version} has no asset for {platform} (expected {expected})"
                )?;
                if available.is_empty() {
                    write!(f, "; that release publishes no assets at all")
                } else {
                    write!(f, "; published assets: {}", available.join(", "))
                }
            }
            Self::NoRelease { channel } => write!(f, "no {channel} release has been published"),
        }
    }
}

impl std::error::Error for VersionCheckError {}

/// A version check: the version string, or why there isn't one.
pub type VersionResult = std::result::Result<String, VersionCheckError>;

/// Fetches the latest release version from GitHub.
/// For beta builds, fetches the latest prerelease; for stable builds,
/// fetches the latest stable release. This ensures each channel only
/// sees updates from its own channel. Releases whose CI hasn't yet
/// uploaded the current-platform binary are skipped — see
/// `release_has_current_platform_asset`.
pub fn fetch_latest_version() -> Option<String> {
    try_fetch_latest_version().ok()
}

/// Fetches the latest version for this channel, or why there is none.
///
/// # Errors
///
/// See [`try_fetch_latest_stable_version`] and [`try_fetch_latest_beta_version`].
pub fn try_fetch_latest_version() -> VersionResult {
    if is_beta() {
        try_fetch_latest_beta_version()
    } else {
        try_fetch_latest_stable_version()
    }
}

/// Fetches the latest stable release version from GitHub.
///
/// Returns `None` for every failure alike; use
/// [`try_fetch_latest_stable_version`] where the reason matters.
pub fn fetch_latest_stable_version() -> Option<String> {
    try_fetch_latest_stable_version().ok()
}

/// Fetches the latest stable release version, or why there is none.
///
/// # Errors
///
/// [`VersionCheckError::Unreachable`] when the releases API cannot be reached
/// or its response cannot be parsed, [`VersionCheckError::NoAssetForPlatform`]
/// when GitHub answers but the release carries no asset for this platform.
pub fn try_fetch_latest_stable_version() -> VersionResult {
    let agent = agent_with_timeout(FETCH_TIMEOUT);
    let release: GitHubRelease = agent
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", "tokensave")
        .call()
        .map_err(unreachable)?
        .body_mut()
        .read_json()
        .map_err(unreachable)?;
    select_stable(&release)
}

/// Fetches the latest prerelease version from GitHub.
///
/// Returns `None` for every failure alike; use
/// [`try_fetch_latest_beta_version`] where the reason matters.
pub fn fetch_latest_beta_version() -> Option<String> {
    try_fetch_latest_beta_version().ok()
}

/// Fetches the latest installable prerelease version, or why there is none.
///
/// # Errors
///
/// As [`try_fetch_latest_stable_version`], plus
/// [`VersionCheckError::NoRelease`] when the beta channel is empty.
pub fn try_fetch_latest_beta_version() -> VersionResult {
    let agent = agent_with_timeout(FETCH_TIMEOUT);
    let releases: Vec<GitHubRelease> = agent
        .get(GITHUB_RELEASES_LIST_URL)
        .header("User-Agent", "tokensave")
        .call()
        .map_err(unreachable)?
        .body_mut()
        .read_json()
        .map_err(unreachable)?;
    select_beta(releases)
}

/// Picks the version out of the latest stable release.
fn select_stable(release: &GitHubRelease) -> VersionResult {
    if release_has_current_platform_asset(release) {
        return Ok(release.tag_name.trim_start_matches('v').to_string());
    }
    Err(no_asset_error(release))
}

/// Picks the newest prerelease whose current-platform asset is already up.
///
/// GitHub returns the list newest-first, so the first match is the latest
/// installable beta. A release whose CI is still uploading is skipped and
/// picked up on a later check; if none of them is installable, the newest
/// prerelease is the one worth naming in the error.
fn select_beta(releases: Vec<GitHubRelease>) -> VersionResult {
    let mut newest: Option<GitHubRelease> = None;
    for release in releases.into_iter().filter(|r| r.prerelease) {
        if release_has_current_platform_asset(&release) {
            return Ok(release.tag_name.trim_start_matches('v').to_string());
        }
        if newest.is_none() {
            newest = Some(release);
        }
    }
    match newest {
        Some(release) => Err(no_asset_error(&release)),
        None => Err(VersionCheckError::NoRelease { channel: "beta" }),
    }
}

/// Builds the "nothing for your platform" error describing `release`.
fn no_asset_error(release: &GitHubRelease) -> VersionCheckError {
    let version = release.tag_name.trim_start_matches('v').to_string();
    VersionCheckError::NoAssetForPlatform {
        expected: asset_name(&version, release.prerelease),
        platform: current_platform().to_string(),
        available: release.assets.iter().map(|a| a.name.clone()).collect(),
        version,
    }
}

/// Wraps a transport or decoding failure as [`VersionCheckError::Unreachable`].
fn unreachable(error: impl std::fmt::Display) -> VersionCheckError {
    VersionCheckError::Unreachable {
        detail: error.to_string(),
    }
}

/// Returns true if the current build is a beta/prerelease version.
pub fn is_beta() -> bool {
    env!("CARGO_PKG_VERSION").contains('-')
}

/// Parses a version string into `(major, minor, patch, pre-release)`.
///
/// Handles optional pre-release suffixes (e.g. `"2.5.0-beta.1"`) by splitting
/// on the first `-`. Returns `None` when the base version is malformed.
fn parse_version(v: &str) -> Option<(u64, u64, u64, Option<&str>)> {
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

/// Classification of an upgrade between two tokensave versions.
///
/// The variant drives the automatic maintenance tokensave performs on upgrade:
///
/// - [`BumpKind::Patch`] (`x.y.Z`): bug fixes only — no reinstall, no reindex.
/// - [`BumpKind::Minor`] (`x.Y.0`): new MCPs/tools — global agent reinstall, no reindex.
/// - [`BumpKind::Major`] (`X.0.0`): DB/schema changes — global reinstall **and** a
///   per-project forced reindex (`sync -f` equivalent).
/// - [`BumpKind::None`]: equal versions, downgrades, or cross-channel transitions —
///   no action beyond advancing the recorded version marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    /// No actionable change (equal, downgrade, or cross-channel).
    None,
    /// Patch bump (`x.y.Z`): bug fixes only.
    Patch,
    /// Minor bump (`x.Y.0`): new MCPs/tools, warrants a global reinstall.
    Minor,
    /// Major bump (`X.0.0`): DB changes, warrants reinstall + forced reindex.
    Major,
}

/// Classifies the upgrade from `old` to `new` as patch, minor, major, or none.
///
/// Uses the same semver rules and channel handling as [`is_newer_version`]:
/// beta and stable are separate channels and never cross. A `new` version that
/// is not strictly newer than `old` (equal or a downgrade) yields
/// [`BumpKind::None`]. An empty or unparseable `old` version is treated as a
/// [`BumpKind::Major`] bump so pre-versioned projects backfill on first use.
///
/// # Examples
///
/// ```
/// use tokensave::cloud::{bump_kind, BumpKind};
/// assert_eq!(bump_kind("6.4.4", "7.0.0"), BumpKind::Major);
/// assert_eq!(bump_kind("6.4.4", "6.5.0"), BumpKind::Minor);
/// assert_eq!(bump_kind("6.4.4", "6.4.5"), BumpKind::Patch);
/// assert_eq!(bump_kind("6.4.4", "6.4.4"), BumpKind::None);
/// ```
pub fn bump_kind(old: &str, new: &str) -> BumpKind {
    // Empty/unparseable old version: treat as needing a full refresh, but only
    // when `new` itself parses and is on the same (stable-vs-stable) channel.
    let Some((nm, nn, np, npre)) = parse_version(new) else {
        return BumpKind::None;
    };
    let Some((om, on, op, opre)) = parse_version(old) else {
        // Cross-channel "old" can't be reasoned about; only backfill when the
        // running version is on the same channel kind we'd otherwise expect.
        return if npre.is_none() {
            BumpKind::Major
        } else {
            BumpKind::None
        };
    };

    // Beta and stable are separate channels — never cross them.
    if opre.is_some() != npre.is_some() {
        return BumpKind::None;
    }

    if !is_newer_version(old, new) {
        return BumpKind::None;
    }

    if nm != om {
        BumpKind::Major
    } else if nn != on {
        BumpKind::Minor
    } else if np != op {
        BumpKind::Patch
    } else {
        // Same base version, strictly-newer pre-release tag within the same
        // channel: classify by base (patch level), matching the channel rules.
        BumpKind::Patch
    }
}

/// Returns true if `latest` is strictly newer than `current` using semver comparison.
/// Handles pre-release suffixes (e.g. "2.5.0-beta.1") by stripping them for the
/// base version comparison, then comparing pre-release tags lexicographically.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = parse_version;

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

    /// `RootCerts::PlatformVerifier` only takes effect under the rustls
    /// provider; a provider/feature mismatch would panic at agent-construction
    /// time rather than at the call site, which would otherwise surface only
    /// as an unexplained crash on a user's first HTTPS call.
    #[test]
    fn agent_with_timeout_builds_with_platform_roots() {
        let _ = agent_with_timeout(Duration::from_secs(1));
    }

    fn cfg(
        pending: u64,
        last_upload_at: i64,
        last_attempt_at: i64,
    ) -> crate::user_config::UserConfig {
        crate::user_config::UserConfig {
            upload_enabled: true,
            pending_upload: pending,
            last_upload_at,
            last_flush_attempt_at: last_attempt_at,
            ..crate::user_config::UserConfig::default()
        }
    }

    const DAY: i64 = UPLOAD_INTERVAL_SECS;

    #[test]
    fn upload_is_due_only_once_a_day() {
        let now = 10 * DAY;
        assert!(
            !upload_is_due(&cfg(500, now - 60, 0), now),
            "an upload a minute ago must not trigger another"
        );
        assert!(
            !upload_is_due(&cfg(500, now - (DAY - 1), 0), now),
            "one second short of a day is not due"
        );
        assert!(
            upload_is_due(&cfg(500, now - DAY, now - DAY), now),
            "a full day since the last success is due"
        );
    }

    #[test]
    fn upload_is_due_on_a_machine_that_has_never_uploaded() {
        // `last_upload_at` is 0 until the first success, which must read as due
        // rather than as "uploaded at the epoch, wait a day".
        assert!(upload_is_due(&cfg(500, 0, 0), 10 * DAY));
    }

    #[test]
    fn nothing_to_send_or_opted_out_is_never_due() {
        let now = 10 * DAY;
        assert!(
            !upload_is_due(&cfg(0, 0, 0), now),
            "no pending tokens means no request"
        );
        let mut opted_out = cfg(500, 0, 0);
        opted_out.upload_enabled = false;
        assert!(!upload_is_due(&opted_out, now), "opt-out is honored");
    }

    #[test]
    fn a_failed_attempt_backs_off_briefly_then_retries() {
        let now = 10 * DAY;
        // A failure leaves `last_upload_at` behind `last_flush_attempt_at`, so
        // the daily gate is still open — the cooldown is the only thing
        // stopping a request per command while the network is down.
        let just_failed = cfg(500, now - 5 * DAY, now - 1);
        assert!(!upload_is_due(&just_failed, now));

        let failed_a_while_ago = cfg(500, now - 5 * DAY, now - FAILED_ATTEMPT_COOLDOWN_SECS);
        assert!(
            upload_is_due(&failed_a_while_ago, now),
            "the cooldown must expire, or a single failure would wedge uploads for a day"
        );
    }

    #[test]
    fn update_check_defaults_on_and_honors_off_values() {
        assert!(update_check_enabled_from(None));
        assert!(update_check_enabled_from(Some("on")));
        assert!(update_check_enabled_from(Some("1")));
        assert!(update_check_enabled_from(Some("please")));
        for off in [
            "off", "OFF", " Off ", "false", "0", "no", "disable", "disabled",
        ] {
            assert!(
                !update_check_enabled_from(Some(off)),
                "{off} should disable"
            );
        }
    }

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

    // --- #513: a missing asset must be distinguishable from an unreachable host ---

    #[test]
    fn stable_selection_reports_missing_asset_not_unreachable() {
        // v7.11.1 as published: every platform but this one. The old code
        // returned None here, which `upgrade` spelled "could not reach GitHub".
        // The real v7.11.1 asset list, minus whichever entry matches the
        // platform this test happens to run on.
        let mine = asset_name("7.11.1", false);
        let published: Vec<String> = [
            "tokensave-v7.11.1-aarch64-linux.tar.gz",
            "tokensave-v7.11.1-aarch64-macos.tar.gz",
            "tokensave-v7.11.1-x86_64-linux.tar.gz",
        ]
        .iter()
        .map(|n| (*n).to_string())
        .filter(|n| *n != mine)
        .collect();
        let names: Vec<&str> = published.iter().map(String::as_str).collect();
        let r = release("v7.11.1", false, &names);
        let err = select_stable(&r).expect_err("no asset for this platform");
        match &err {
            VersionCheckError::NoAssetForPlatform {
                version, available, ..
            } => {
                assert_eq!(version, "7.11.1");
                assert_eq!(available.len(), names.len());
            }
            other => panic!("expected NoAssetForPlatform, got {other:?}"),
        }
        let msg = err.to_string();
        assert!(msg.contains("7.11.1"), "{msg}");
        assert!(msg.contains(current_platform()), "{msg}");
        assert!(
            !msg.contains("could not reach"),
            "a missing asset must not be reported as a network fault: {msg}"
        );
    }

    #[test]
    fn stable_selection_returns_version_when_asset_present() {
        let r = release("v9.9.9", false, &[&asset_name("9.9.9", false)]);
        assert_eq!(select_stable(&r).unwrap(), "9.9.9");
    }

    #[test]
    fn beta_selection_skips_incomplete_release_for_older_complete_one() {
        // Newest beta is still uploading; the one before it is installable.
        let releases = vec![
            release("v9.9.9-beta.2", true, &[]),
            release("v9.9.9-beta.1", true, &[&asset_name("9.9.9-beta.1", true)]),
        ];
        assert_eq!(select_beta(releases).unwrap(), "9.9.9-beta.1");
    }

    #[test]
    fn beta_selection_reports_newest_prerelease_when_none_installable() {
        let releases = vec![
            release(
                "v9.9.9-beta.2",
                true,
                &["tokensave-beta-v9.9.9-beta.2-other.tar.gz"],
            ),
            release("v9.9.8", false, &[&asset_name("9.9.8", false)]),
        ];
        match select_beta(releases).expect_err("no installable beta") {
            VersionCheckError::NoAssetForPlatform { version, .. } => {
                assert_eq!(version, "9.9.9-beta.2");
            }
            other => panic!("expected NoAssetForPlatform, got {other:?}"),
        }
    }

    #[test]
    fn beta_selection_reports_no_release_when_channel_is_empty() {
        let releases = vec![release("v9.9.8", false, &[&asset_name("9.9.8", false)])];
        assert!(matches!(
            select_beta(releases).expect_err("no betas at all"),
            VersionCheckError::NoRelease { channel: "beta" }
        ));
    }

    #[test]
    fn unreachable_keeps_the_network_wording_and_carries_the_cause() {
        let err = VersionCheckError::Unreachable {
            detail: "http status: 503".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("could not reach GitHub"), "{msg}");
        assert!(
            msg.contains("503"),
            "the underlying cause is worth surfacing: {msg}"
        );
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
