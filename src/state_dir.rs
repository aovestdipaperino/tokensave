//! Relocatable project state (#919).
//!
//! By default a project's state lives in `<root>/.tokensave`. A base state
//! directory, from `TOKENSAVE_STATE_DIR` or else `state_dir` in
//! `~/.tokensave/config.toml`, moves it to `<base>/projects/<id>`, so nothing
//! is written inside the working tree. Prerelease builds use a `beta`
//! subdirectory in either layout, as they do today.
//!
//! `<id>` is derived from the canonical, normalized absolute path of the
//! project root: the root's folder name followed by a short SHA-256 of the
//! path. The same root always gets the same id, and two linked worktrees of
//! one repository, which have different roots, get different ids.
//!
//! Existing state is never stranded by a configuration change:
//!
//! * a project that already has state under its root keeps using it after a
//!   base is configured;
//! * every base that has held state is recorded in
//!   `~/.tokensave/state-bases.json`, and state found under a recorded base
//!   is used when the configured base (or the default) has none.
//!
//! `tokensave doctor` reports state that is not where the configuration puts
//! it, and relocated state whose project root no longer exists. Nothing here
//! moves or deletes state; `tokensave state prune` deletes orphans on request.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Environment variable naming the base state directory. It takes precedence
/// over `state_dir` in `config.toml`; set but empty, it selects the default
/// in-repository layout.
pub const STATE_DIR_ENV: &str = "TOKENSAVE_STATE_DIR";

/// The `config.toml` key naming the base state directory.
pub const STATE_DIR_KEY: &str = "state_dir";

/// The directory under a base that holds one directory per project. Keeping
/// projects one level down means a base equal to `~/.tokensave` cannot
/// collide with the global files there.
pub const PROJECTS_DIR: &str = "projects";

/// File recording which project root a relocated state directory belongs to.
pub const ORIGIN_FILENAME: &str = "origin.json";

/// File in `~/.tokensave` listing every base that has held project state.
const BASES_FILENAME: &str = "state-bases.json";

/// Where the base state directory setting came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseSource {
    /// `TOKENSAVE_STATE_DIR`.
    Env,
    /// `state_dir` in `~/.tokensave/config.toml`.
    UserConfig,
    /// Neither is set: state lives under each project root.
    Default,
}

/// The effective base state directory setting, for reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseSetting {
    pub source: BaseSource,
    /// The value as written, when one was set.
    pub raw: Option<String>,
    /// The base in use: `None` for the default layout, or when `raw` is not
    /// an absolute path (see [`parse_base`]).
    pub base: Option<PathBuf>,
}

/// Where a project's state lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// `<root>/.tokensave`, the default layout.
    InRepo(PathBuf),
    /// `<base>/projects/<id>`.
    Relocated { base: PathBuf, dir: PathBuf },
}

impl Location {
    /// The channel-specific state directory.
    pub fn dir(&self) -> &Path {
        match self {
            Self::InRepo(dir) | Self::Relocated { dir, .. } => dir,
        }
    }

    pub fn into_dir(self) -> PathBuf {
        match self {
            Self::InRepo(dir) | Self::Relocated { dir, .. } => dir,
        }
    }
}

/// Contents of [`ORIGIN_FILENAME`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// The project root, canonical and normalized (see [`canonical_root`]).
    pub project_root: String,
    /// UNIX timestamp of when the state directory was first recorded.
    pub created_at: i64,
    /// The tokensave version that recorded it.
    pub version: String,
}

/// Reads the base state directory setting: `TOKENSAVE_STATE_DIR` when set,
/// otherwise `state_dir` in `~/.tokensave/config.toml`, otherwise the default.
///
/// The variable is read on every call. The `config.toml` value is read once
/// per process, like the other settings a running server loads at start, so
/// the many path lookups of a session do not reread the file.
pub fn setting() -> BaseSetting {
    if let Some(value) = std::env::var_os(STATE_DIR_ENV) {
        let raw = value.to_string_lossy().into_owned();
        return BaseSetting {
            source: BaseSource::Env,
            base: parse_base(&raw),
            raw: Some(raw),
        };
    }
    user_config_setting().clone()
}

/// The base in use, or `None` for the default layout.
pub fn configured_base() -> Option<PathBuf> {
    setting().base
}

fn user_config_setting() -> &'static BaseSetting {
    static SETTING: OnceLock<BaseSetting> = OnceLock::new();
    SETTING.get_or_init(|| {
        // Read from the same home the rest of the user settings use for
        // decisions about project paths (see `auto_init::setting`).
        let raw = crate::agents::home_dir()
            .map(|home| home.join(".tokensave").join("config.toml"))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|contents| contents.parse::<toml::Table>().ok())
            .and_then(|table| {
                table
                    .get(STATE_DIR_KEY)
                    .and_then(toml::Value::as_str)
                    .map(str::to_string)
            });
        match raw {
            Some(raw) => BaseSetting {
                source: BaseSource::UserConfig,
                base: parse_base(&raw),
                raw: Some(raw),
            },
            None => BaseSetting {
                source: BaseSource::Default,
                raw: None,
                base: None,
            },
        }
    })
}

/// Parses a base state directory value. Empty selects the default layout. A
/// leading `~` is the home directory. Anything else must be absolute: a
/// relative base would move with the current directory, so it is ignored and
/// `tokensave doctor` reports it.
pub fn parse_base(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let path = if raw == "~" {
        crate::agents::home_dir()?
    } else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        crate::agents::home_dir()?.join(rest)
    } else {
        PathBuf::from(raw)
    };
    path.is_absolute().then_some(path)
}

/// The canonical, normalized absolute form of `root`: symlinks resolved when
/// the path exists, Windows verbatim prefixes removed, no trailing separator.
pub fn canonical_root(root: &Path) -> PathBuf {
    let path = std::fs::canonicalize(root)
        .or_else(|_| std::path::absolute(root))
        .unwrap_or_else(|_| root.to_path_buf());
    let mut text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            text = format!(r"\\{rest}");
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            text = rest.to_string();
        }
    }
    while text.len() > 1 && (text.ends_with('/') || text.ends_with('\\')) && !text.ends_with(":\\")
    {
        text.pop();
    }
    PathBuf::from(text)
}

/// The string `root`'s id is derived from: [`canonical_root`], lowercased on
/// Windows, whose file systems ignore case.
pub fn project_key(root: &Path) -> String {
    let key = canonical_root(root).to_string_lossy().into_owned();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

/// The directory name of `root` under `<base>/projects`: the root's folder
/// name, limited to portable characters, then 16 hex digits of the SHA-256 of
/// [`project_key`]. The name is only for people browsing the directory; the
/// hash alone identifies the project.
pub fn project_id(root: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let key = project_key(root);
    let name: String = canonical_root(root)
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let name = name.trim_matches('.');
    let name = if name.is_empty() { "project" } else { name };
    let digest = Sha256::digest(key.as_bytes());
    let hash = digest.iter().take(8).fold(String::new(), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    });
    format!("{name}-{hash}")
}

/// `<base>/projects/<id>`, without the channel subdirectory.
pub fn relocated_root(base: &Path, root: &Path) -> PathBuf {
    base.join(PROJECTS_DIR).join(project_id(root))
}

/// The state directory for `root` on the given channel. `in_repo` is the
/// default-layout directory, `channel` adds the channel subdirectory to a
/// relocated root.
pub(crate) fn locate(
    root: &Path,
    in_repo: PathBuf,
    channel: impl Fn(PathBuf) -> PathBuf,
) -> Location {
    let configured = configured_base();
    let known = known_bases();
    // The default layout with no recorded bases is the common case, and it
    // needs no file system access at all.
    if configured.is_none() && known.is_empty() {
        return Location::InRepo(in_repo);
    }
    let relocated = |base: &Path| channel(relocated_root(base, root));
    if let Some(base) = configured.as_deref() {
        let dir = relocated(base);
        if holds_project(&dir) {
            return Location::Relocated {
                base: base.to_path_buf(),
                dir,
            };
        }
    }
    if holds_project(&in_repo) {
        return Location::InRepo(in_repo);
    }
    for base in known {
        if configured.as_deref() == Some(base.as_path()) {
            continue;
        }
        let dir = relocated(base);
        if holds_project(&dir) {
            return Location::Relocated {
                base: base.clone(),
                dir,
            };
        }
    }
    match configured {
        Some(base) => Location::Relocated {
            dir: relocated(&base),
            base,
        },
        None => Location::InRepo(in_repo),
    }
}

/// Whether `dir` holds a project's state: a database or a project config.
pub fn holds_project(dir: &Path) -> bool {
    dir.join("tokensave.db").exists() || dir.join("config.json").is_file()
}

/// `~/.tokensave/state-bases.json`. Not channel-specific: the bases are a
/// user setting, which stable and beta share.
fn bases_path() -> Option<PathBuf> {
    crate::agents::home_dir().map(|home| home.join(".tokensave").join(BASES_FILENAME))
}

fn read_bases(path: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<Vec<PathBuf>>(&contents).ok())
        .unwrap_or_default()
}

/// The recorded bases, read once per process. A base recorded later by this
/// process is the configured one, which [`locate`] checks first anyway.
pub fn known_bases() -> &'static [PathBuf] {
    static BASES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    BASES.get_or_init(|| {
        bases_path()
            .map(|path| read_bases(&path))
            .unwrap_or_default()
    })
}

/// Records that `root` keeps its state under `base`: writes
/// [`ORIGIN_FILENAME`] when it is missing and adds `base` to the recorded
/// bases. Best effort: a failure leaves the state usable, and only `doctor`
/// and the recorded-base fallback lose sight of it.
pub fn record(base: &Path, root: &Path) {
    let dir = relocated_root(base, root);
    let origin_path = dir.join(ORIGIN_FILENAME);
    if !origin_path.exists() {
        let origin = Origin {
            project_root: canonical_root(root).to_string_lossy().into_owned(),
            created_at: crate::tokensave::current_timestamp(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&origin) {
            if std::fs::create_dir_all(&dir).is_ok() {
                let tmp = origin_path.with_extension(format!("{}.tmp", std::process::id()));
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(&tmp, &origin_path);
                }
            }
        }
    }
    let Some(path) = bases_path() else {
        return;
    };
    let mut bases = read_bases(&path);
    if bases.iter().any(|known| known == base) {
        return;
    }
    bases.push(base.to_path_buf());
    let Ok(json) = serde_json::to_string_pretty(&bases) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Per-process temporary name, so two processes recording at once never
    // write the same file. The read-modify-write itself is not locked: a base
    // lost to a race is recorded again the next time a project under it saves
    // its config, and until then that base is still used while configured.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// One `<base>/projects/<id>` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEntry {
    pub base: PathBuf,
    pub dir: PathBuf,
    /// The recorded origin, or `None` when [`ORIGIN_FILENAME`] is missing or
    /// unreadable.
    pub origin: Option<Origin>,
}

impl StateEntry {
    /// Relocated state whose recorded project root no longer exists.
    pub fn is_orphaned(&self) -> bool {
        self.origin
            .as_ref()
            .is_some_and(|origin| !Path::new(&origin.project_root).exists())
    }
}

/// Every relocated state directory under the configured base and the
/// recorded bases, sorted by path.
pub fn scan() -> Vec<StateEntry> {
    let mut bases: Vec<PathBuf> = known_bases().to_vec();
    if let Some(base) = configured_base() {
        if !bases.contains(&base) {
            bases.push(base);
        }
    }
    let mut entries = Vec::new();
    for base in bases {
        let Ok(read) = std::fs::read_dir(base.join(PROJECTS_DIR)) else {
            continue;
        };
        for entry in read.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let dir = entry.path();
            let origin = std::fs::read_to_string(dir.join(ORIGIN_FILENAME))
                .ok()
                .and_then(|contents| serde_json::from_str::<Origin>(&contents).ok());
            entries.push(StateEntry {
                base: base.clone(),
                dir,
                origin,
            });
        }
    }
    entries.sort_by(|a, b| a.dir.cmp(&b.dir));
    entries
}

/// Total size in bytes of the files under `dir`, without following symlinks.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut work = vec![dir.to_path_buf()];
    while let Some(next) = work.pop() {
        let Ok(read) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in read.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                work.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().map_or(0, |meta| meta.len());
            }
        }
    }
    total
}

/// Deletes the orphaned entries of [`scan`] (see [`StateEntry::is_orphaned`]),
/// or only lists them when `dry_run` is set. Returns each orphan with the
/// error that kept it, if any. Entries without a recorded origin are never
/// deleted: nothing says which project they belong to.
pub fn prune(dry_run: bool) -> Vec<(StateEntry, Option<std::io::Error>)> {
    scan()
        .into_iter()
        .filter(StateEntry::is_orphaned)
        .map(|entry| {
            let error = if dry_run {
                None
            } else {
                std::fs::remove_dir_all(&entry.dir).err()
            };
            (entry, error)
        })
        .collect()
}

/// Project roots with relocated state whose root still exists.
pub fn relocated_projects() -> Vec<PathBuf> {
    scan()
        .into_iter()
        .filter_map(|entry| entry.origin)
        .map(|origin| PathBuf::from(origin.project_root))
        .filter(|root| root.exists())
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_or_empty_base_is_not_used() {
        assert_eq!(parse_base(""), None);
        assert_eq!(parse_base("   "), None);
        assert_eq!(parse_base("relative/state"), None);
        let absolute = std::env::temp_dir().join("tokensave-state");
        assert_eq!(
            parse_base(&absolute.to_string_lossy()),
            Some(absolute.clone())
        );
    }

    #[test]
    fn the_same_root_always_gets_the_same_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("My Repo");
        std::fs::create_dir_all(&root).unwrap();
        let id = project_id(&root);
        assert_eq!(id, project_id(&root));
        // A trailing separator or a `.` component names the same directory.
        assert_eq!(id, project_id(&root.join(".")));
        let name = id.rsplit_once('-').map(|(name, _)| name);
        assert_eq!(name, Some("My_Repo"));
        assert_eq!(id.rsplit_once('-').map(|(_, hash)| hash.len()), Some(16));
    }

    #[test]
    fn different_roots_get_different_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("repo");
        let linked = tmp.path().join("worktrees").join("repo");
        std::fs::create_dir_all(&main).unwrap();
        std::fs::create_dir_all(&linked).unwrap();
        assert_ne!(project_id(&main), project_id(&linked));
    }

    #[test]
    fn projects_sit_below_the_base_so_they_cannot_collide_with_global_files() {
        let base = Path::new("/base");
        let dir = relocated_root(base, Path::new("/work/global.db"));
        assert_eq!(dir.parent(), Some(base.join(PROJECTS_DIR).as_path()));
    }
}
