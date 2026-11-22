//! Self-update for the tokensave binary.
//!
//! Downloads the latest release asset directly from GitHub, extracts the
//! binary, and replaces the running executable using `self_replace`.
//! Beta and stable are separate channels — a beta build only sees beta
//! releases and vice versa.

use std::collections::HashSet;
use std::path::Path;

use crate::cloud;
use crate::errors::{Result, TokenSaveError};
use crate::purge::provenance::{self, InstallMethod, Provenance};
use crate::user_config::UserConfig;

const GITHUB_REPO: &str = "aovestdipaperino/tokensave";

// Asset-naming and platform helpers live in `crate::cloud` so the version-
// detection path can use the same naming convention to filter out releases
// whose CI hasn't finished uploading the current platform's binary yet.
use crate::cloud::asset_name;
#[cfg(test)]
use crate::cloud::current_platform;

/// The GitHub release tag for a given version.
fn release_tag(version: &str) -> String {
    format!("v{version}")
}

fn io_err(msg: &str) -> impl Fn(std::io::Error) -> TokenSaveError + '_ {
    move |e| TokenSaveError::Config {
        message: format!("{msg}: {e}"),
    }
}

/// The name of the release asset listing each other asset's SHA256.
///
/// Published by both release workflows in `sha256sum` format, one line per
/// asset.
const SUMS_ASSET: &str = "SHA256SUMS";

/// One asset attached to a GitHub release.
#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// A located release asset, together with the SHA256 the release publishes
/// for it.
struct VerifiedAsset {
    url: String,
    sha256: String,
}

/// Fetches every asset attached to a release.
///
/// One request serves both the binary lookup and the `SHA256SUMS` lookup —
/// they come from the same release, so asking twice would only add a way for
/// the two to disagree.
fn fetch_release_assets(tag: &str) -> Result<Vec<Asset>> {
    #[derive(serde::Deserialize)]
    struct Release {
        assets: Vec<Asset>,
    }

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/tags/{tag}");
    let agent = cloud::agent_with_timeout(std::time::Duration::from_secs(30));

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

    Ok(release.assets)
}

/// Downloads a small text asset (the sums file) in full.
fn fetch_text_asset(url: &str) -> Result<String> {
    let agent = cloud::agent_with_timeout(std::time::Duration::from_secs(30));
    agent
        .get(url)
        .header("User-Agent", "tokensave")
        .call()
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to download {SUMS_ASSET}: {e}"),
        })?
        .body_mut()
        .read_to_string()
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {SUMS_ASSET}: {e}"),
        })
}

/// Pulls one asset's hash out of a `sha256sum`-format listing.
///
/// Accepts both the text (`hash  name`) and binary (`hash *name`) markers
/// `sha256sum` emits, and ignores blank lines so a trailing newline is not an
/// error. Returns `None` when the file is well-formed but does not mention
/// `asset`, which the caller reports separately from a malformed file.
fn sha256_for_asset(listing: &str, asset: &str) -> Option<String> {
    listing.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start_matches(['*', ' ']).trim();
        (name == asset && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Locates the platform's release asset and the SHA256 published for it.
///
/// Fails closed (#525). A release whose `SHA256SUMS` is missing, unreadable,
/// or silent about this asset aborts the upgrade rather than installing an
/// unverified binary: `upgrade` only ever moves forward to a channel's latest,
/// so every release a verifying binary can reach is one published after the
/// sums file started shipping. Treating "no sums" as "skip verification" would
/// hand anyone who can suppress the file a silent downgrade to no integrity
/// check at all, which is the property being bought here.
fn locate_verified_asset(tag: &str, expected_asset: &str) -> Result<VerifiedAsset> {
    let assets = fetch_release_assets(tag)?;

    let url = assets
        .iter()
        .find(|a| a.name == expected_asset)
        .map(|a| a.browser_download_url.clone())
        .ok_or_else(|| TokenSaveError::Config {
            message: format!(
                "release {tag} exists but asset '{expected_asset}' is not yet available.\n  \
                 CI build may still be in progress — try again in a few minutes.\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })?;

    let sums_url = assets
        .iter()
        .find(|a| a.name == SUMS_ASSET)
        .map(|a| a.browser_download_url.clone())
        .ok_or_else(|| TokenSaveError::Update {
            message: format!(
                "release {tag} publishes no {SUMS_ASSET}, so the download cannot be \
                 verified — refusing to install.\n  \
                 If CI is still finishing, try again in a few minutes:\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })?;

    let listing = fetch_text_asset(&sums_url)?;
    let sha256 =
        sha256_for_asset(&listing, expected_asset).ok_or_else(|| TokenSaveError::Update {
            message: format!(
                "release {tag} publishes {SUMS_ASSET} but it lists no hash for \
                 '{expected_asset}' — refusing to install.\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })?;

    Ok(VerifiedAsset { url, sha256 })
}

/// Hex-encodes the SHA256 of `data`.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut acc, byte| {
            use std::fmt::Write;
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}

/// A downloaded binary staged for installation, and the private directory
/// holding it.
///
/// The directory is owned by this value so it is removed when the staged
/// binary goes out of scope, whether the replacement succeeded or not.
struct StagedBinary {
    /// Held for its `Drop`, which removes the directory and anything left in
    /// it. Never read directly — [`Self::path`] is the accessor.
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl StagedBinary {
    fn path(&self) -> &Path {
        &self.path
    }
}

/// Creates the private directory the downloaded binary is extracted into.
///
/// The destination used to be `std::env::temp_dir().join("tokensave_upgrade_<pid>")`
/// — a name another local user can predict and pre-create. Both extraction
/// paths open it with `create`-like semantics that follow symlinks, so on a
/// host with a shared world-writable `/tmp` a symlink planted at that path
/// would be followed and the target written with this process's privileges
/// (#525). `TempDir` picks a random name and creates it exclusively, failing
/// rather than adopting a path that already exists, which is what defeats the
/// planted symlink; the extraction code is then unchanged and simply writes
/// somewhere nobody else can name. The mode is narrowed to `0700` afterwards
/// because `tempdir()` creates with `0777 & !umask` — commonly `0755`, which
/// would leave the staged binary readable by every other local user between
/// extraction and install.
fn new_staging_dir() -> Result<tempfile::TempDir> {
    let dir = tempfile::Builder::new()
        .prefix("tokensave_upgrade_")
        .tempdir()
        .map_err(io_err("create temp dir failed"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .map_err(io_err("secure temp dir failed"))?;
    }

    Ok(dir)
}

/// Downloads the archive from `url` into memory, then extracts `bin_name`
/// into a freshly created private directory. Returns the staged binary.
fn download_and_extract(url: &str, bin_name: &str, expected_sha256: &str) -> Result<StagedBinary> {
    let tmp_dir = new_staging_dir()?;
    let tmp_path = tmp_dir.path().join(format!(
        "tokensave{}",
        if cfg!(windows) { ".exe" } else { "" }
    ));

    let agent = cloud::agent_with_timeout(std::time::Duration::from_mins(5));

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

    // Verified before extraction, not after (#525): extraction writes the
    // archive's contents to disk, so a mismatched archive must never reach it.
    eprint!("  Verifying...");
    let actual = sha256_hex(&raw);
    if actual != expected_sha256 {
        eprintln!(" \x1b[31mFAILED\x1b[0m");
        return Err(TokenSaveError::Update {
            message: format!(
                "downloaded archive does not match the SHA256 published for it — \
                 refusing to install.\n  \
                 expected: {expected_sha256}\n  \
                 actual:   {actual}\n  \
                 This means the download was corrupted or tampered with. \
                 Nothing has been installed."
            ),
        });
    }
    eprintln!(" OK");

    eprint!("  Extracting...");

    #[cfg(not(windows))]
    extract_targz(&raw, bin_name, &tmp_path)?;

    #[cfg(windows)]
    extract_zip(&raw, bin_name, &tmp_path)?;

    eprintln!(" Done");
    Ok(StagedBinary {
        _dir: tmp_dir,
        path: tmp_path,
    })
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

        // zip 9 decodes the entry name on demand and reports undecodable names
        // as an error.
        let name = file.name().map_err(|e| TokenSaveError::Config {
            message: format!("zip entry name error: {e}"),
        })?;
        if Path::new(name.as_ref())
            .file_name()
            .and_then(|n| n.to_str())
            == Some(bin_name)
        {
            let mut out = std::fs::File::create(dest).map_err(io_err("create temp file failed"))?;
            std::io::copy(&mut file, &mut out).map_err(io_err("extract failed"))?;
            return Ok(());
        }
    }

    Err(TokenSaveError::Config {
        message: format!("binary '{bin_name}' not found in zip"),
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
fn install_binary(src: &Path, target: &Path, published: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let dir = target.parent().ok_or_else(|| TokenSaveError::Config {
        message: "cannot determine target directory".into(),
    })?;
    let temp = tempfile::NamedTempFile::new_in(dir).map_err(io_err("cannot stage new binary"))?;
    std::fs::copy(src, temp.path()).map_err(io_err("cannot copy new binary"))?;
    temp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))
        .map_err(io_err("cannot set executable mode"))?;
    verify_standalone_bytes(target, published)?;
    temp.persist(target)
        .map_err(|error| io_err("cannot replace binary")(error.error))?;
    Ok(())
}

/// Downloads, extracts, and installs the binary for `version`/`is_beta`.
/// Verifies the release asset exists on GitHub and returns the download URL.
/// Call this early so we fail fast when CI hasn't finished building the
/// release yet.
fn preflight_asset_check(version: &str, is_beta: bool) -> Result<VerifiedAsset> {
    let tag = release_tag(version);
    let expected = asset_name(version, is_beta);
    eprintln!("  Asset: {expected}");
    // Resolving the hash here rather than mid-download keeps the fail-fast
    // property the asset check already had: a release missing its sums fails
    // before the archive is fetched, not after ~150 MiB of it.
    locate_verified_asset(&tag, &expected)
}

/// Record the *currently running* binary's version in user config just before
/// the binary is replaced. The new binary reads this on startup as
/// `previous_version` and decides whether reinstall is required for the
/// transition (e.g. minor/major bumps re-register agents to pick up new MCP
/// tools or hook changes; patch bumps just update the field).
fn record_previous_version() {
    let current = env!("CARGO_PKG_VERSION");
    let mut cfg = UserConfig::load();
    if cfg.previous_version == current {
        return;
    }
    cfg.previous_version = current.to_string();
    if !cfg.save() {
        eprintln!(
            "  \x1b[33mwarning:\x1b[0m could not record previous version; \
             run `tokensave reinstall` manually if new tools aren't registered"
        );
    }
}

fn stage_release(asset: &VerifiedAsset) -> Result<StagedBinary> {
    let bin_name = if cfg!(windows) {
        "tokensave.exe"
    } else {
        "tokensave"
    };
    download_and_extract(&asset.url, bin_name, &asset.sha256)
}

fn binary_digest(path: &Path) -> Result<sha2::digest::Output<sha2::Sha256>> {
    use sha2::{Digest, Sha256};
    use std::io::BufRead;

    let file = std::fs::File::open(path).map_err(io_err("cannot read executable"))?;
    let mut reader = std::io::BufReader::new(file);
    let mut hash = Sha256::new();
    loop {
        let chunk = reader
            .fill_buf()
            .map_err(io_err("cannot hash executable"))?;
        if chunk.is_empty() {
            break;
        }
        hash.update(chunk);
        let count = chunk.len();
        reader.consume(count);
    }
    Ok(hash.finalize())
}

fn verify_standalone_bytes(executable: &Path, published: &Path) -> Result<()> {
    if binary_digest(executable)? == binary_digest(published)? {
        return Ok(());
    }
    Err(TokenSaveError::Update {
        message: "running executable differs from the published GitHub release; upgrade with the original installer".into(),
    })
}

fn standalone_target(installation: &Provenance) -> Result<&Path> {
    installation
        .executable
        .as_deref()
        .filter(|path| {
            installation.method == InstallMethod::Unknown && !provenance::may_be_managed(path)
        })
        .ok_or_else(|| TokenSaveError::Update {
            message: "the executable may be package-managed; use the original installer".into(),
        })
}

fn replace_verified_standalone(path: &Path, published: &Path, staged: &Path) -> Result<()> {
    let installation = provenance::detect();
    if standalone_target(&installation)? != path {
        return Err(TokenSaveError::Update {
            message: "executable location changed during the upgrade".into(),
        });
    }
    #[cfg(unix)]
    install_binary(staged, path, published)?;
    #[cfg(not(unix))]
    {
        verify_standalone_bytes(path, published)?;
        self_replace::self_replace(staged).map_err(io_err("binary replacement failed"))?;
    }
    record_previous_version();
    Ok(())
}

/// A running tokensave process other than this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProcess {
    pub pid: u32,
    /// Command line (or process name when the command line is unavailable).
    pub description: String,
}

/// Strip a case-insensitive `.exe` suffix from a Windows process name,
/// without panicking when the last 4 bytes of `name` don't fall on a UTF-8
/// character boundary.
///
/// Compiled for `test` too (not just `windows`) so the boundary-safety
/// regression test can exercise this logic on any host platform.
#[cfg(any(windows, test))]
fn strip_windows_exe_suffix(name: &str) -> &str {
    let len = name.len();
    if len < 4 || !name.is_char_boundary(len - 4) {
        return name;
    }
    let (stem, suffix) = name.split_at(len - 4);
    if suffix.eq_ignore_ascii_case(".exe") {
        stem
    } else {
        name
    }
}

/// Is `stem` (a basename with any `.exe` suffix already stripped) the
/// tokensave binary, using Windows matching semantics?
///
/// Windows treats file and process names as case-insensitive, so basenames
/// like `TokenSave` or `TOKENSAVE` refer to the same binary as `tokensave`.
///
/// Compiled for `test` too (not just `windows`) so the regression test can
/// exercise Windows matching semantics on any host platform.
#[cfg(any(windows, test))]
fn is_tokensave_stem_windows(stem: &str) -> bool {
    stem.eq_ignore_ascii_case("tokensave")
}

/// Is `name` the tokensave binary (platform-dependent extension included)?
fn is_tokensave_process_name(name: &str) -> bool {
    #[cfg(windows)]
    {
        let stem = strip_windows_exe_suffix(name);
        is_tokensave_stem_windows(stem)
    }
    #[cfg(not(windows))]
    {
        let stem = name.strip_suffix(".exe").unwrap_or(name);
        stem == "tokensave"
    }
}

/// A reduced, pure row used for kill-candidate selection: just enough of a
/// process snapshot (pid, parent pid, name, description) to reason about
/// ancestry without touching sysinfo directly. Keeping this separate from
/// `RunningProcess` lets the selection logic be unit-tested with a
/// hand-built table instead of the real process tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessEntry {
    pid: u32,
    parent: Option<u32>,
    name: String,
    description: String,
}

/// PIDs to exclude from kill candidates: this process plus every reachable
/// ancestor found by walking `parent` links in `table`.
///
/// The walk stops as soon as it can no longer make progress safely: the
/// current pid has no entry in `table`, its parent is absent or `0`, or the
/// parent has already been visited (a cycle or self-parent). This guards a
/// Scoop shim (or any other launcher) named `tokensave`/`tokensave.exe` from
/// being offered as a killable "unrelated" process, even when it launched us
/// through an intermediate non-tokensave process (e.g. a shell).
fn self_and_ancestor_pids(table: &[ProcessEntry], self_pid: u32) -> HashSet<u32> {
    let mut excluded = HashSet::new();
    let mut current = Some(self_pid);

    while let Some(pid) = current {
        if pid == 0 || !excluded.insert(pid) {
            break;
        }
        current = table.iter().find(|e| e.pid == pid).and_then(|e| e.parent);
    }

    excluded
}

/// Reduce a process table to the tokensave processes safe to offer as kill
/// candidates: exclude this process and every reachable ancestor, keep the
/// rest that look like tokensave, and preserve pid ordering.
fn kill_candidates(table: &[ProcessEntry], self_pid: u32) -> Vec<RunningProcess> {
    let excluded = self_and_ancestor_pids(table, self_pid);

    let mut found: Vec<RunningProcess> = table
        .iter()
        .filter(|e| !excluded.contains(&e.pid) && is_tokensave_process_name(&e.name))
        .map(|e| RunningProcess {
            pid: e.pid,
            description: e.description.clone(),
        })
        .collect();
    found.sort_by_key(|p| p.pid);
    found
}

/// List running tokensave processes, excluding this one and its ancestors.
///
/// These are mostly MCP servers spawned by editors/agents. They hold the old
/// binary's DB handles open, so upgrading under them can leave a server
/// running code that no longer matches the on-disk index format.
fn find_running_processes() -> Vec<RunningProcess> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let self_pid = std::process::id();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cmd(sysinfo::UpdateKind::Always),
    );

    let table: Vec<ProcessEntry> = sys
        .processes()
        .values()
        .map(|p| {
            let cmd: Vec<String> = p
                .cmd()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let name = p.name().to_string_lossy().into_owned();
            let description = if cmd.is_empty() {
                name.clone()
            } else {
                cmd.join(" ")
            };
            ProcessEntry {
                pid: p.pid().as_u32(),
                parent: p.parent().map(sysinfo::Pid::as_u32),
                name,
                description,
            }
        })
        .collect();

    kill_candidates(&table, self_pid)
}

/// Terminate `procs`; returns the number successfully signalled.
fn kill_processes(procs: &[RunningProcess]) -> usize {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let pids: Vec<Pid> = procs.iter().map(|p| Pid::from_u32(p.pid)).collect();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::new(),
    );

    let mut killed = 0;
    for proc in procs {
        let pid = Pid::from_u32(proc.pid);
        match sys.process(pid) {
            Some(p) if p.kill() => {
                eprintln!("  \x1b[32m✔\x1b[0m killed pid {}", proc.pid);
                killed += 1;
            }
            Some(_) => eprintln!("  \x1b[33m!\x1b[0m failed to kill pid {}", proc.pid),
            // Already gone between listing and killing — treat as success.
            None => killed += 1,
        }
    }
    killed
}

/// Decide whether the upgrade may proceed after attempting to stop blocking
/// tokensave processes.
///
/// v7.8.0 defines "stopped" as *successfully signalled*: this does not wait
/// for the OS to finish tearing a process down, and it does not rescan for
/// respawned processes. If fewer processes were signalled than requested,
/// the upgrade is aborted rather than proceeding while an old binary may
/// still be running.
fn post_kill_outcome(requested: usize, killed: usize) -> Result<()> {
    if killed >= requested {
        return Ok(());
    }
    Err(TokenSaveError::Config {
        message: format!(
            "upgrade stopped: only {killed}/{requested} running tokensave process(es) could be stopped.\n  \
             Stop the remaining process(es) manually, then retry the upgrade.",
        ),
    })
}

/// Handle tokensave processes that are still running before an upgrade.
///
/// With `kill` set, terminate them without asking. Otherwise ask, but only
/// when there is something to kill and stdin is a TTY; a non-interactive run
/// just warns and continues. If a kill was requested (via `--kill` or an
/// interactive `y`) and not every process could be stopped, the upgrade is
/// aborted rather than proceeding under a still-running old binary.
fn handle_running_processes(kill: bool) -> Result<()> {
    use std::io::{IsTerminal, Write};

    let procs = find_running_processes();
    if procs.is_empty() {
        return Ok(());
    }

    eprintln!("{} other tokensave process(es) running:", procs.len());
    for p in &procs {
        eprintln!("  • pid {} — {}", p.pid, p.description);
    }

    if !kill {
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "  \x1b[33m!\x1b[0m Continuing without killing them (non-interactive; use --kill)."
            );
            return Ok(());
        }
        eprint!("  Kill them before upgrading? [y/N] ");
        std::io::stderr().flush().ok();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return Ok(());
        }
        if !answer.trim().eq_ignore_ascii_case("y") {
            eprintln!("  Leaving them running.");
            return Ok(());
        }
    }

    let requested = procs.len();
    let killed = kill_processes(&procs);
    eprintln!("Killed {killed}/{requested} process(es).");
    post_kill_outcome(requested, killed)
}

/// Show installer-specific guidance or upgrade a verified standalone release.
pub fn run_upgrade(kill: bool) -> Result<String> {
    run_upgrade_with_options(kill, false, false)
}

/// Run or preview an upgrade using the requested standalone policy.
pub fn run_upgrade_with_options(kill: bool, standalone: bool, dry_run: bool) -> Result<String> {
    let current = env!("CARGO_PKG_VERSION");
    let beta = cloud::is_beta();
    let installation = provenance::detect();
    eprintln!(
        "Current version: v{current} ({}, {})",
        channel_name(beta),
        installation.method.label()
    );
    eprintln!("  Evidence: {}", installation.evidence);
    if let Some(path) = &installation.executable {
        eprintln!("  Executable: {}", path.display());
    }
    if installation.method != InstallMethod::Unknown || !standalone {
        eprintln!("{}", provenance::upgrade_guidance(&installation, beta));
        return Ok(current.to_string());
    }
    let path = standalone_target(&installation)?;
    if dry_run {
        eprintln!("Plan: verify the running binary against the current GitHub release, then replace it only if a newer {} release is available.", channel_name(beta));
        return Ok(current.to_string());
    }

    let latest = cloud::try_fetch_latest_version().map_err(|error| TokenSaveError::Update {
        message: format!("failed to check for updates: {error}"),
    })?;
    let UpgradeStatus::UpgradeAvailable(latest) = classify_upgrade(current, &latest) else {
        eprintln!("Already up to date (v{current}).");
        return Ok(current.to_string());
    };
    eprintln!("Verifying standalone release before upgrading v{current} to v{latest}...");
    upgrade_standalone(path, latest, beta, kill)?;
    eprintln!("Successfully upgraded to v{latest}.");
    Ok(latest.to_string())
}

fn upgrade_standalone(path: &Path, version: &str, beta: bool, kill: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let published = stage_release(&preflight_asset_check(current, cloud::is_beta())?)?;
    verify_standalone_bytes(path, published.path())?;
    let staged = stage_release(&preflight_asset_check(version, beta)?)?;
    handle_running_processes(kill)?;
    replace_verified_standalone(path, published.path(), staged.path())
}

fn channel_name(beta: bool) -> &'static str {
    if beta {
        "beta"
    } else {
        "stable"
    }
}

/// Print the current channel.
pub fn show_channel() {
    eprintln!(
        "v{} ({})",
        env!("CARGO_PKG_VERSION"),
        channel_name(cloud::is_beta())
    );
}

/// Channel switching has the same installer policy as upgrading.
pub fn switch_channel(target_channel: &str) -> Result<String> {
    switch_channel_with_options(target_channel, false)
}

/// Switch release channels for an explicitly selected standalone install.
pub fn switch_channel_with_options(target_channel: &str, standalone: bool) -> Result<String> {
    let current = env!("CARGO_PKG_VERSION");
    let beta = match target_channel {
        "beta" => true,
        "stable" => false,
        _ => {
            return Err(TokenSaveError::Config {
                message: format!(
                    "unknown channel '{target_channel}'. Valid channels: stable, beta"
                ),
            })
        }
    };
    if beta == cloud::is_beta() {
        eprintln!("Already on the {target_channel} channel (v{current}).");
        return Ok(current.to_string());
    }
    let installation = provenance::detect();
    if installation.method != InstallMethod::Unknown || !standalone {
        return Err(TokenSaveError::Update {
            message: format!(
                "Cannot switch this installation in place to {target_channel}. Use your original installer's channel or install the requested channel separately. {}",
                provenance::upgrade_guidance(&installation, cloud::is_beta()),
            ),
        });
    }
    let path = standalone_target(&installation)?;
    let latest = if beta {
        cloud::try_fetch_latest_beta_version()
    } else {
        cloud::try_fetch_latest_stable_version()
    }
    .map_err(|error| TokenSaveError::Update {
        message: format!("cannot switch to the {target_channel} channel: {error}"),
    })?;
    upgrade_standalone(path, &latest, beta, false)?;
    eprintln!("Switched to {target_channel} channel: v{latest}");
    Ok(latest)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls
)]
mod tests {
    use super::*;

    #[test]
    fn standalone_requires_current_release_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("installed");
        let published = dir.path().join("published");
        std::fs::write(&installed, b"standalone release").unwrap();
        std::fs::write(&published, b"standalone release").unwrap();
        assert!(verify_standalone_bytes(&installed, &published).is_ok());
        std::fs::write(&installed, b"locally rebuilt").unwrap();
        assert!(verify_standalone_bytes(&installed, &published).is_err());
        assert_eq!(std::fs::read(installed).unwrap(), b"locally rebuilt");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_homebrew_executable_stays_out_of_standalone_path_with_or_without_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let keg = dir.path().join("Cellar").join("tokensave").join("7.17.0");
        let bin = keg.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let executable = bin.join("tokensave");
        std::fs::write(&executable, b"token").unwrap();
        let link = dir.path().join("tokensave");
        std::os::unix::fs::symlink(&executable, &link).unwrap();
        let without_receipt = provenance::detect_from(&link, None);
        assert_eq!(without_receipt.method, InstallMethod::Unknown);
        assert!(standalone_target(&without_receipt).is_err());
        std::fs::write(
            keg.join("INSTALL_RECEIPT.json"),
            r#"{"source":{"tap":"aovestdipaperino/tap"}}"#,
        )
        .unwrap();
        let with_receipt = provenance::detect_from(&link, None);
        assert_eq!(with_receipt.method, InstallMethod::Homebrew);
        assert!(standalone_target(&with_receipt).is_err());
    }

    /// The staging directory must not be at a name another local user can
    /// predict. The old path was `temp_dir()/tokensave_upgrade_<pid>`, and a
    /// pid is both guessable and observable, so a symlink planted there ahead
    /// of the download would have been followed on extraction (#525).
    #[test]
    fn the_staging_directory_is_not_a_predictable_path() {
        let a = new_staging_dir().expect("staging dir");
        let b = new_staging_dir().expect("staging dir");

        assert_ne!(
            a.path(),
            b.path(),
            "two staging directories in the same process must not collide"
        );

        let legacy = std::env::temp_dir().join(format!("tokensave_upgrade_{}", std::process::id()));
        assert_ne!(a.path(), legacy, "must not reuse the old pid-derived name");
    }

    /// `TempDir` must create the directory rather than adopt one that is
    /// already there — adopting is what makes a planted path dangerous.
    #[test]
    fn the_staging_directory_is_freshly_created() {
        let dir = new_staging_dir().expect("staging dir");
        assert!(dir.path().is_dir(), "the directory must exist");
        assert!(
            std::fs::read_dir(dir.path())
                .expect("read staging dir")
                .next()
                .is_none(),
            "a freshly created staging directory must be empty"
        );
    }

    /// On Unix the directory must not be readable or writable by other users,
    /// so nothing can race the extracted binary between write and install.
    /// `tempdir()` alone creates with `0777 & !umask` — commonly `0755`, which
    /// leaves the staged binary readable by every other local user while it
    /// sits there. The narrowing is therefore explicit, and asserted.
    #[cfg(unix)]
    #[test]
    fn the_staging_directory_is_private_to_this_user() {
        use std::os::unix::fs::PermissionsExt;

        let dir = new_staging_dir().expect("staging dir");
        let mode = std::fs::metadata(dir.path())
            .expect("stat staging dir")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(
            mode, 0o700,
            "staging directory should be user-only, got {mode:o}"
        );
    }

    /// The staged binary sits inside that directory, so its full path inherits
    /// the directory's unpredictability even though the file name is fixed.
    #[test]
    fn the_staged_binary_lives_inside_the_private_directory() {
        let dir = new_staging_dir().expect("staging dir");
        let staged = dir.path().join(format!(
            "tokensave{}",
            if cfg!(windows) { ".exe" } else { "" }
        ));

        assert!(
            staged.starts_with(dir.path()),
            "the staged binary must be contained by the private directory"
        );
    }

    // ── SHA256 verification (#525) ───────────────────────────────────────

    #[test]
    fn sha256_hex_matches_a_known_vector() {
        // The empty-input digest, so this pins the encoding rather than
        // restating whatever the implementation happens to produce.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_sums_listing_yields_the_hash_for_the_named_asset() {
        let listing = "\
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  tokensave-v1.2.3-x86_64-linux.tar.gz
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  tokensave-v1.2.3-aarch64-macos.tar.gz
";
        assert_eq!(
            sha256_for_asset(listing, "tokensave-v1.2.3-aarch64-macos.tar.gz").as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[test]
    fn a_sums_listing_accepts_the_binary_mode_marker() {
        // `sha256sum -b` and the Windows leg emit `hash *name`.
        let listing =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 *tokensave-v1.2.3-x86_64-windows.zip\n";
        assert_eq!(
            sha256_for_asset(listing, "tokensave-v1.2.3-x86_64-windows.zip").as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }

    #[test]
    fn an_asset_absent_from_the_listing_has_no_hash() {
        // The caller turns this into a refusal, not a skipped check — a
        // release that does not name our asset must not install (#525).
        let listing =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  some-other-asset.tar.gz\n";
        assert!(sha256_for_asset(listing, "tokensave-v1.2.3-x86_64-linux.tar.gz").is_none());
    }

    #[test]
    fn a_malformed_or_truncated_hash_is_not_accepted() {
        // A short, over-long, or non-hex field must not be treated as a hash:
        // it would compare unequal to every real digest and turn a verifiable
        // release into an unexplained mismatch.
        for bad in [
            "deadbeef  tokensave-v1.2.3-x86_64-linux.tar.gz",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz  tokensave-v1.2.3-x86_64-linux.tar.gz",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855aa  tokensave-v1.2.3-x86_64-linux.tar.gz",
        ] {
            assert!(
                sha256_for_asset(bad, "tokensave-v1.2.3-x86_64-linux.tar.gz").is_none(),
                "{bad} should not parse as a hash"
            );
        }
    }

    #[test]
    fn an_empty_or_blank_listing_has_no_hash() {
        for listing in ["", "\n", "   \n\n"] {
            assert!(sha256_for_asset(listing, "tokensave-v1.2.3-x86_64-linux.tar.gz").is_none());
        }
    }

    #[test]
    fn a_hash_is_compared_case_insensitively() {
        // Some tools emit uppercase hex; normalising on parse keeps that from
        // reading as tampering.
        let listing =
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855  tokensave-v1.2.3-x86_64-linux.tar.gz\n";
        assert_eq!(
            sha256_for_asset(listing, "tokensave-v1.2.3-x86_64-linux.tar.gz").as_deref(),
            Some(sha256_hex(b"").as_str())
        );
    }

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
    fn tokensave_process_names_are_recognized() {
        assert!(is_tokensave_process_name("tokensave"));
        assert!(is_tokensave_process_name("tokensave.exe"));
        assert!(!is_tokensave_process_name("tokensave-helper"));
        assert!(!is_tokensave_process_name("cargo"));
        assert!(!is_tokensave_process_name(""));
    }

    #[test]
    fn strip_windows_exe_suffix_does_not_panic_on_unicode_boundary() {
        // Each '€' is a 3-byte UTF-8 sequence, so this 6-byte, 2-char name
        // has `len - 4 == 2`, a byte offset that falls *inside* the first
        // '€' rather than on a char boundary. The naive `name[len - 4..]`
        // slice used to panic here even though the name has nothing to do
        // with tokensave or `.exe`.
        let name = "€€";
        assert_eq!(strip_windows_exe_suffix(name), name);
    }

    #[test]
    fn is_tokensave_process_name_ignores_unrelated_unicode_names() {
        assert!(!is_tokensave_process_name("€€"));
    }

    #[test]
    fn is_tokensave_stem_windows_is_ascii_case_insensitive() {
        // Windows file/process names are case-insensitive: mixed-case
        // basenames like `TokenSave.EXE` or `TOKENSAVE.exe` (after the
        // `.exe` suffix has already been stripped) must still be recognized
        // as the tokensave binary under Windows matching semantics, even
        // though the Unix comparison stays exact/case-sensitive.
        assert!(is_tokensave_stem_windows(strip_windows_exe_suffix(
            "TokenSave.EXE"
        )));
        assert!(is_tokensave_stem_windows(strip_windows_exe_suffix(
            "TOKENSAVE.exe"
        )));
        assert!(is_tokensave_stem_windows("tokensave"));
        assert!(!is_tokensave_stem_windows("tokensave-helper"));
    }

    #[test]
    fn find_running_processes_excludes_self() {
        let self_pid = std::process::id();
        assert!(find_running_processes().iter().all(|p| p.pid != self_pid));
    }

    fn entry(pid: u32, parent: Option<u32>, name: &str, description: &str) -> ProcessEntry {
        ProcessEntry {
            pid,
            parent,
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    #[test]
    fn kill_candidates_excludes_scoop_shim_parent_launching_self() {
        // A Scoop shim named tokensave.exe launched us directly.
        let table = [
            entry(50, None, "tokensave.exe", r"C:\shims\tokensave.exe upgrade"),
            entry(100, Some(50), "tokensave.exe", "tokensave.exe upgrade"),
        ];
        assert!(kill_candidates(&table, 100).is_empty());
    }

    #[test]
    fn kill_candidates_excludes_tokensave_grandparent_behind_shell() {
        // self -> bash -> tokensave (e.g. a launcher script invoked via a shell).
        let table = [
            entry(40, None, "tokensave", "tokensave"),
            entry(60, Some(40), "bash", "bash"),
            entry(100, Some(60), "tokensave", "tokensave upgrade"),
        ];
        assert!(kill_candidates(&table, 100).is_empty());
    }

    #[test]
    fn kill_candidates_keeps_unrelated_server_excludes_launcher_ancestor() {
        let table = [
            entry(50, None, "tokensave.exe", r"C:\shims\tokensave.exe upgrade"),
            entry(100, Some(50), "tokensave", "tokensave upgrade --kill"),
            entry(200, None, "tokensave", "tokensave serve"),
        ];
        let candidates = kill_candidates(&table, 100);
        assert_eq!(
            candidates,
            vec![RunningProcess {
                pid: 200,
                description: "tokensave serve".to_string(),
            }]
        );
    }

    #[test]
    fn self_and_ancestor_pids_terminates_on_self_parent_cycle() {
        let table = [entry(100, Some(100), "tokensave", "tokensave")];
        let excluded = self_and_ancestor_pids(&table, 100);
        assert_eq!(excluded, HashSet::from([100]));
    }

    #[test]
    fn self_and_ancestor_pids_terminates_on_two_cycle() {
        let table = [
            entry(100, Some(200), "tokensave", "tokensave"),
            entry(200, Some(100), "tokensave", "tokensave"),
        ];
        let excluded = self_and_ancestor_pids(&table, 100);
        assert_eq!(excluded, HashSet::from([100, 200]));
    }

    #[test]
    fn kill_candidates_excludes_lookalikes_and_preserves_pid_order() {
        let table = [
            entry(400, None, "tokensave", "tokensave serve two"),
            entry(300, None, "tokensave-helper", "tokensave-helper"),
            entry(250, None, "tokensave", "tokensave serve"),
        ];
        let candidates = kill_candidates(&table, 100);
        assert_eq!(
            candidates,
            vec![
                RunningProcess {
                    pid: 250,
                    description: "tokensave serve".to_string(),
                },
                RunningProcess {
                    pid: 400,
                    description: "tokensave serve two".to_string(),
                },
            ]
        );
    }

    #[test]
    fn self_and_ancestor_pids_stops_on_missing_parent_entry() {
        // self's parent pid (999) has no entry in the table. The walk records
        // it (harmless — it can never match a real table row) and then halts
        // because there is nothing further to look up.
        let table = [entry(100, Some(999), "bash", "bash")];
        let excluded = self_and_ancestor_pids(&table, 100);
        assert_eq!(excluded, HashSet::from([100, 999]));
    }

    #[test]
    fn self_and_ancestor_pids_stops_on_zero_parent() {
        let table = [entry(100, Some(0), "bash", "bash")];
        let excluded = self_and_ancestor_pids(&table, 100);
        assert_eq!(excluded, HashSet::from([100]));
    }

    #[test]
    fn self_and_ancestor_pids_stops_when_self_has_no_entry() {
        // self_pid has no corresponding row in the table at all.
        let table = [entry(1, None, "bash", "bash")];
        let excluded = self_and_ancestor_pids(&table, 100);
        assert_eq!(excluded, HashSet::from([100]));
    }

    #[test]
    fn self_and_ancestor_pids_walks_a_deep_chain() {
        let table = [
            entry(1, None, "init", "init"),
            entry(10, Some(1), "bash", "bash"),
            entry(20, Some(10), "bash", "bash"),
            entry(30, Some(20), "tokensave", "tokensave"),
            entry(100, Some(30), "tokensave", "tokensave upgrade"),
        ];
        let excluded = self_and_ancestor_pids(&table, 100);
        assert_eq!(excluded, HashSet::from([100, 30, 20, 10, 1]));
    }

    #[test]
    #[cfg(unix)]
    fn live_snapshot_includes_self_and_real_parent_pid() {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        let self_pid = std::process::id();
        let real_ppid = std::os::unix::process::parent_id();

        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new().with_cmd(sysinfo::UpdateKind::Always),
        );

        let self_proc = sys
            .process(sysinfo::Pid::from_u32(self_pid))
            .expect("current process must be present in its own snapshot");
        assert_eq!(
            self_proc.parent().map(|p| p.as_u32()),
            Some(real_ppid),
            "sysinfo's reported parent must match the real OS parent pid"
        );
    }

    #[test]
    fn post_kill_outcome_succeeds_when_all_requested_were_killed() {
        assert!(post_kill_outcome(3, 3).is_ok());
    }

    #[test]
    fn post_kill_outcome_succeeds_when_nothing_was_requested() {
        assert!(post_kill_outcome(0, 0).is_ok());
    }

    #[test]
    fn post_kill_outcome_fails_on_partial_kill() {
        let err = post_kill_outcome(3, 2).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains('2'),
            "message should include killed count: {message}"
        );
        assert!(
            message.contains('3'),
            "message should include requested count: {message}"
        );
        assert!(
            message.to_lowercase().contains("upgrade") && message.to_lowercase().contains("stop"),
            "message should explain the upgrade stopped: {message}"
        );
        assert!(
            message.to_lowercase().contains("manually") && message.to_lowercase().contains("retry"),
            "message should tell the user to stop them manually and retry: {message}"
        );
    }

    #[test]
    fn post_kill_outcome_fails_on_total_kill_failure() {
        let err = post_kill_outcome(3, 0).unwrap_err();
        let message = err.to_string();
        assert!(message.contains('0'));
        assert!(message.contains('3'));
    }

    #[test]
    #[cfg(unix)]
    fn kill_processes_tolerates_dead_pids() {
        // A process that has already exited counts as killed, not as a failure.
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        child.kill().unwrap();
        child.wait().unwrap();

        let killed = kill_processes(&[RunningProcess {
            pid,
            description: "sleep 30".to_string(),
        }]);
        assert_eq!(killed, 1);
    }

    #[test]
    #[cfg(unix)]
    fn kill_processes_kills_a_live_child() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let killed = kill_processes(&[RunningProcess {
            pid: child.id(),
            description: "sleep 30".to_string(),
        }]);
        assert_eq!(killed, 1);
        // Reap so the test leaves no zombie behind.
        child.wait().ok();
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

    #[cfg(unix)]
    #[test]
    fn verified_standalone_replacement_preserves_a_separate_install() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("tokensave");
        let published = dir.path().join("published");
        let staged = dir.path().join("staged");
        let other = dir.path().join("other-install");
        std::fs::write(&installed, b"old release").unwrap();
        std::fs::write(&published, b"old release").unwrap();
        std::fs::write(&staged, b"new release").unwrap();
        std::fs::write(&other, b"package-managed").unwrap();

        install_binary(&staged, &installed, &published).unwrap();
        assert_eq!(std::fs::read(&installed).unwrap(), b"new release");
        assert_eq!(std::fs::read(&other).unwrap(), b"package-managed");
        assert_eq!(
            std::fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 4);

        // A second updater that verified the previous release cannot replace it again.
        assert!(install_binary(&staged, &installed, &published).is_err());
    }
}
