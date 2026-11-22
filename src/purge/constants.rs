//! Every constant of `tokensave purge` and install provenance (#1055).
//!
//! The file names below are the entries purge recognizes as tokensave-owned.
//! Purge deletes only entries listed here. Anything else found in a state
//! directory is reported and left in place, so a file another tool or the
//! user put there is never removed because it shares a directory with ours.

/// User settings: `~/.tokensave/config.toml` (shared by both channels).
pub const USER_CONFIG_FILENAME: &str = "config.toml";
/// Machine-local state that is not a setting: `~/.tokensave/state.toml`.
pub const STATE_FILENAME: &str = "state.toml";
/// Hardware benchmark cache: `~/.tokensave/benchmark.toml` (#752).
pub const BENCHMARK_FILENAME: &str = "benchmark.toml";
/// Model pricing cache: `~/.tokensave/pricing.json`.
pub const PRICING_FILENAME: &str = "pricing.json";
/// Memory telemetry ring: `~/.tokensave/memory.mmap`.
pub const MEMORY_MMAP_FILENAME: &str = "memory.mmap";
/// Sync monitor ring: `~/.tokensave/monitor.mmap`.
pub const MONITOR_MMAP_FILENAME: &str = "monitor.mmap";
/// Running-server registry: `~/.tokensave/servers/`.
pub const SERVERS_DIRNAME: &str = "servers";
/// Profiler aggregates: `~/.tokensave/profile/` (#754).
pub const PROFILE_DIRNAME: &str = "profile";
/// Every base that has held relocated project state (#919).
pub const STATE_BASES_FILENAME: &str = "state-bases.json";

/// The global database: token-savings history and the project registry.
pub const GLOBAL_DB_FILENAME: &str = "global.db";
/// SQLite sidecars of [`GLOBAL_DB_FILENAME`].
pub const GLOBAL_DB_SIDECARS: &[&str] = &["global.db-wal", "global.db-shm"];

/// The suffix of the lock file kept next to a settings file.
pub const LOCK_SUFFIX: &str = ".lock";
/// The infix of an atomic-write temporary (`config.tmp.<pid>.<n>`).
pub const ATOMIC_TMP_INFIX: &str = ".tmp.";
/// Settings files whose lock files and atomic-write leftovers purge removes.
pub const LOCKED_SETTINGS: &[&str] = &[USER_CONFIG_FILENAME, STATE_FILENAME, BENCHMARK_FILENAME];

/// The beta channel's subdirectory of a stable state directory.
pub const BETA_DIRNAME: &str = "beta";

/// Files whose presence shows a project state directory is tokensave's.
pub const PROJECT_OWNERSHIP_MARKERS: &[&str] = &[
    "tokensave.db",
    crate::config::CONFIG_FILENAME,
    "branch-meta.json",
];

/// The word an interactive purge must be confirmed with.
pub const CONFIRMATION_WORD: &str = "purge";

/// The `config.toml` key listing installed agents.
pub const INSTALLED_AGENTS_KEY: &str = "installed_agents";

/// How deep the size estimate walks; deeper than any layout tokensave writes.
pub const MAX_SIZE_DEPTH: usize = 32;
/// Units for the plan's size column.
pub const SIZE_UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
/// The step between [`SIZE_UNITS`].
pub const SIZE_STEP: u64 = 1024;

/// Windows `ERROR_SHARING_VIOLATION` and `ERROR_LOCK_VIOLATION`: a file
/// another process (usually a running MCP server) has open.
pub const WINDOWS_FILE_IN_USE: [i32; 2] = [32, 33];

// ---------------------------------------------------------------------------
// Installation provenance (see `provenance.rs`)
// ---------------------------------------------------------------------------

/// The package name every supported installer uses.
pub const PACKAGE_NAME: &str = "tokensave";
/// Homebrew's keg root directory name.
pub const HOMEBREW_CELLAR_DIR: &str = "Cellar";
/// The keg subdirectory that holds executables.
pub const HOMEBREW_BIN_DIR: &str = "bin";
/// Scoop's per-app directory name.
pub const SCOOP_APPS_DIR: &str = "apps";
/// The manifest Scoop writes into every installed app version directory.
pub const SCOOP_MANIFEST_FILENAME: &str = "manifest.json";
/// Cargo's install-root binary directory.
pub const CARGO_BIN_DIR: &str = "bin";
/// The directory under the home directory Cargo uses by default.
pub const CARGO_DEFAULT_HOME_DIR: &str = ".cargo";
/// The environment variable that relocates Cargo's home.
pub const CARGO_HOME_ENV: &str = "CARGO_HOME";
/// Cargo's JSON install manifest (also updated by `cargo binstall`).
pub const CARGO_CRATES2_FILENAME: &str = ".crates2.json";
/// Cargo's original TOML install manifest.
pub const CARGO_CRATES_FILENAME: &str = ".crates.toml";
/// The JSON manifest's table of installed packages.
pub const CARGO_CRATES2_INSTALLS_KEY: &str = "installs";
/// The JSON manifest's list of binaries per package.
pub const CARGO_CRATES2_BINS_KEY: &str = "bins";
/// The TOML manifest's table of installed packages.
pub const CARGO_CRATES_V1_KEY: &str = "v1";

/// Removes a Cargo (or `cargo binstall`) installation.
pub const CARGO_UNINSTALL: &str = "cargo uninstall tokensave";
/// Removes a Homebrew installation.
pub const HOMEBREW_UNINSTALL: &str = "brew uninstall tokensave";
/// Removes a Scoop installation.
pub const SCOOP_UNINSTALL: &str = "scoop uninstall tokensave";

// ---------------------------------------------------------------------------
// `tokensave purge` exit codes
// ---------------------------------------------------------------------------

/// The purge ran but something could not be removed.
pub const EXIT_INCOMPLETE: i32 = 1;
/// The purge was refused or aborted before any change.
pub const EXIT_REFUSED: i32 = 2;
