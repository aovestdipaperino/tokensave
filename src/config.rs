use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glob::Pattern;
use serde::{Deserialize, Serialize};

use crate::errors::{Result, TokenSaveError};

/// Name of the configuration file stored inside the `.tokensave` directory.
pub const CONFIG_FILENAME: &str = "config.json";

/// Name of the hidden directory used to store `TokenSave` metadata.
pub const TOKENSAVE_DIR: &str = ".tokensave";

/// Name of the project-level query-ignore file stored inside the
/// `.tokensave` directory. See [`load_query_ignore`].
pub const QUERYIGNORE_FILENAME: &str = "queryignore";

/// Configuration for a `TokenSave` project.
///
/// Controls which files are indexed, size limits, and feature toggles.
/// Language inclusion is derived automatically from the installed
/// `LanguageExtractor` set — only exclude patterns live in the config.
/// Serde default for [`TokenSaveConfig::docs_dir`], so configs written before
/// #154 keep discovering docs in the conventional location.
fn default_docs_dir() -> String {
    crate::docs::DEFAULT_DOCS_DIR.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSaveConfig {
    /// Schema version of the configuration.
    pub version: u32,
    /// Root directory of the project being indexed.
    pub root_dir: String,
    /// Glob patterns for files to exclude during indexing.
    pub exclude: Vec<String>,
    /// Glob patterns for hidden (dot-prefixed) paths to include despite the
    /// default hidden-directory filter.  For example, `[".github/**"]` indexes
    /// files under `.github/` that would otherwise be skipped.
    #[serde(default)]
    pub include: Vec<String>,
    /// Maximum file size in bytes; files larger than this are skipped.
    pub max_file_size: u64,
    /// Whether to extract doc comments from source files.
    pub extract_docstrings: bool,
    /// Whether to track call-site locations for edges.
    pub track_call_sites: bool,
    /// Whether to respect `.gitignore` rules when scanning files.
    #[serde(default)]
    pub git_ignore: bool,
    /// Default path-include substrings applied to analysis tool queries when
    /// no explicit `path_include` is provided by the caller.
    #[serde(default)]
    pub default_path_include: Vec<String>,
    /// Default path-exclude substrings applied to analysis tool queries when
    /// no explicit `path_exclude` is provided by the caller.
    #[serde(default)]
    pub default_path_exclude: Vec<String>,
    /// Glob patterns for paths that should be treated as production source
    /// even when an ambiguous directory name (such as `test/`) matches the
    /// default test-file heuristic. Explicit test markers such as
    /// `__tests__/`, `*.test.*`, and `*.spec.*` still take precedence.
    #[serde(default)]
    pub source_path_overrides: Vec<String>,
    /// Directory (project-relative) scanned for companion documentation that
    /// declares its coverage via `applies_to` front matter (#154). Defaults to
    /// `tokensave-docs`; set it to relocate the directory, or to an empty
    /// string to disable docs-directory discovery entirely. Sidecar
    /// `*.readme.md` files are unaffected by this setting.
    #[serde(default = "default_docs_dir")]
    pub docs_dir: String,
    /// tokensave version that last fully indexed this project.
    ///
    /// Used to decide whether a major-version upgrade requires a forced
    /// per-project reindex (`sync -f` equivalent). Empty on pre-7.0 projects
    /// and brand-new configs; an empty value is treated as "needs reindex" so
    /// such projects backfill the latest schema on first MCP tool use.
    #[serde(default)]
    pub last_indexed_version: String,
    /// Transparently track the current git branch when it is untracked, by
    /// copying the nearest-ancestor DB on the next `TokenSave::open`. Defaults
    /// to `false` (current behavior: fall back to the ancestor DB with a
    /// warning). The `TOKENSAVE_AUTO_TRACK` env var overrides this per-run.
    /// Independent of the `post-checkout` git hook, which tracks on branch
    /// switch when `tokensave install` set it up.
    #[serde(default)]
    pub auto_track: bool,
    /// Refuse `tokensave_*` MCP calls when the index describes a different
    /// working tree than the one you are in, instead of answering with a
    /// warning attached (#372 §2). Defaults to `false`.
    ///
    /// Two conditions qualify: a borrowed worktree index (#312), and a branch
    /// that drifted under a running server (#400). Both are detected already;
    /// this only decides whether a detection warns or refuses.
    ///
    /// The argument for opting in is that a wrong answer is worse than no
    /// answer: every tool built on top of tokensave — an agent rule saying
    /// "always check tokensave before reading files", say — inherits the
    /// wrong-tree result with no signal that anything is off, and an empty
    /// result reads as "no such symbol". The argument for it staying opt-in is
    /// that a shared index across a family of worktrees is a legitimate setup,
    /// and hard-erroring it would be a bad surprise.
    ///
    /// The diagnostic tools (`status`, `config`, `diagnose`, `diagnostics`)
    /// are never refused, so the refusal stays investigable from inside the
    /// session that hit it.
    #[serde(default)]
    pub strict_tree: bool,
    /// Ceiling on how many files a single *automatic* sync will take on
    /// before refusing (#396, #393). `0` disables the check.
    ///
    /// Applies only to syncs the user did not ask for — the MCP server's
    /// startup catch-up and its per-`tools/call` staleness check. An explicit
    /// `tokensave sync` is unbounded and is the supported way to index a large
    /// change deliberately. The 30 s cooldown bounds how *often* an automatic
    /// sync runs, never what one costs, so the file count is capped as well.
    #[serde(default = "default_max_auto_sync_files")]
    pub max_auto_sync_files: usize,
    /// Surface per-call savings to the agent, so it can report them to the
    /// user. Defaults to `true` (current behavior). When `false`, tool results
    /// omit the `tokensave_metrics:` line and the MCP `instructions` drop the
    /// sentence asking the agent to report savings — the two things that make a
    /// model spend *output* tokens narrating what tokensave saved on *input*
    /// (#356). The `TOKENSAVE_REPORT_SAVINGS` env var overrides this per-run.
    /// Accounting is unaffected either way: savings are still recorded to the
    /// global DB, so `tokensave gain` and `tokensave list` keep working.
    #[serde(default = "default_report_savings")]
    pub report_savings: bool,
    /// Extensions of non-code files tracked by path so `tokensave_files` can
    /// find them (#323). These are never parsed and contribute no symbols; the
    /// point is that a question like "where are the `.feature` files?" has a
    /// graph answer, since the shell alternative is blocked by the hook.
    ///
    /// An extension already handled by a language extractor is ignored here —
    /// the symbol pass owns those files and records them with their symbols.
    #[serde(default = "default_artifact_extensions")]
    pub artifact_extensions: Vec<String>,
    /// Silence the index-scope warning `serve` prints for a home-directory
    /// project or an index past 5 GB (#450). Defaults to `false`.
    ///
    /// The warning is not a refusal: applying #396's cap to an index that
    /// already exists would decide for the user which of their working setups
    /// stop working, and no threshold does that without breaking somebody who
    /// is currently fine. Someone who deliberately indexes a very large tree
    /// is not wrong, only unusual — this is the switch that says so once
    /// instead of on every server start.
    #[serde(default)]
    pub suppress_scope_warning: bool,
}

/// Serde default for [`TokenSaveConfig::artifact_extensions`].
///
/// Deliberately narrow: these are the formats that carry project meaning and
/// are looked up by path — specifications, schemas, fixtures, and docs. Adding
/// every text extension would turn `tokensave_files` into a directory listing.
fn default_artifact_extensions() -> Vec<String> {
    [
        "feature", "json", "yaml", "yml", "sql", "toml", "proto", "graphql", "md",
    ]
    .iter()
    .map(|ext| (*ext).to_string())
    .collect()
}

/// Serde default for [`TokenSaveConfig::report_savings`], so configs written
/// before #356 keep reporting savings rather than silently going quiet.
fn default_report_savings() -> bool {
    true
}

/// Serde default for [`TokenSaveConfig::max_auto_sync_files`], so configs
/// written before #396 gain the bound instead of staying unbounded.
fn default_max_auto_sync_files() -> usize {
    crate::tokensave::DEFAULT_MAX_AUTO_SYNC_FILES
}

/// Resolves a boolean setting that an environment variable may override.
///
/// Presence of `var` enables the setting unless its value is explicitly falsey
/// (`0`, `false`, `no`, `off`, or empty); absence falls back to `config_value`.
/// This is the convention `TOKENSAVE_AUTO_TRACK` established.
pub fn env_bool_override(var: &str, config_value: bool) -> bool {
    match std::env::var(var) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off" | ""
        ),
        Err(_) => config_value,
    }
}

impl Default for TokenSaveConfig {
    fn default() -> Self {
        Self {
            version: 1,
            root_dir: String::new(),
            exclude: vec![
                // Tool/output/state dirs are matched at any depth (`**/`), so a
                // parent root_dir with nested projects still excludes each
                // project's own build/state dirs (#173).
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
                "**/.tokensave/**".to_string(),
                "**/node_modules/**".to_string(),
                "**/vendor/**".to_string(),
                "**/*.min.*".to_string(),
                "**/build/**".to_string(),
                "**/out/**".to_string(),
                "**/.gradle/**".to_string(),
                // Large language-toolchain trees that aren't project source and
                // aren't covered by `.gitignore` outside a repo (#174).
                "**/site-packages/**".to_string(),
                "**/.venv/**".to_string(),
                "**/venv/**".to_string(),
                "**/__pycache__/**".to_string(),
                "bin/**".to_string(),
            ],
            include: Vec::new(),
            max_file_size: 1_048_576,
            extract_docstrings: true,
            track_call_sites: true,
            git_ignore: true,
            default_path_include: Vec::new(),
            default_path_exclude: Vec::new(),
            source_path_overrides: Vec::new(),
            docs_dir: default_docs_dir(),
            last_indexed_version: String::new(),
            auto_track: false,
            strict_tree: false,
            max_auto_sync_files: default_max_auto_sync_files(),
            report_savings: default_report_savings(),
            artifact_extensions: default_artifact_extensions(),
            suppress_scope_warning: false,
        }
    }
}

/// Returns the path to the `.tokensave` directory within the given project root.
pub fn get_tokensave_dir(project_root: &Path) -> PathBuf {
    project_root.join(TOKENSAVE_DIR)
}

/// Returns the path to the configuration file (`config.json`) within the `.tokensave` directory.
pub fn get_config_path(project_root: &Path) -> PathBuf {
    get_tokensave_dir(project_root).join(CONFIG_FILENAME)
}

/// Loads the configuration from disk.
///
/// If the configuration file does not exist, returns a default configuration
/// with `root_dir` set to the given project root.
pub fn load_config(project_root: &Path) -> Result<TokenSaveConfig> {
    let config_path = get_config_path(project_root);

    if !config_path.exists() {
        return Ok(TokenSaveConfig {
            root_dir: project_root.to_string_lossy().to_string(),
            ..TokenSaveConfig::default()
        });
    }

    let contents = fs::read_to_string(&config_path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to read config file '{}': {}",
            config_path.display(),
            e
        ),
    })?;

    let config: TokenSaveConfig =
        serde_json::from_str(&contents).map_err(|e| TokenSaveError::Config {
            message: format!(
                "failed to parse config file '{}': {}",
                config_path.display(),
                e
            ),
        })?;

    Ok(config)
}

/// Saves the configuration to disk using an atomic write.
///
/// Writes to a temporary file first and then renames it to the final location,
/// ensuring that a partial write never corrupts the configuration.
pub fn save_config(project_root: &Path, config: &TokenSaveConfig) -> Result<()> {
    let tokensave_dir = get_tokensave_dir(project_root);
    fs::create_dir_all(&tokensave_dir).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to create tokensave directory '{}': {}",
            tokensave_dir.display(),
            e
        ),
    })?;

    let config_path = get_config_path(project_root);
    let tmp_path = config_path.with_extension("tmp");

    let json = serde_json::to_string_pretty(config).map_err(|e| TokenSaveError::Config {
        message: format!("failed to serialize config: {e}"),
    })?;

    fs::write(&tmp_path, &json).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to write temporary config file '{}': {}",
            tmp_path.display(),
            e
        ),
    })?;

    fs::rename(&tmp_path, &config_path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to rename temporary config file '{}' to '{}': {}",
            tmp_path.display(),
            config_path.display(),
            e
        ),
    })?;

    Ok(())
}

/// Returns `true` if `.tokensave` is ignored by Git for this project.
///
/// This respects the repository `.gitignore`, `.git/info/exclude`, and the
/// user's global excludes file via `git check-ignore`. If Git cannot answer
/// (for example outside a Git repository), falls back to checking the local
/// `.gitignore` file only.
pub fn is_in_gitignore(project_path: &Path) -> bool {
    if let Some(is_ignored) = is_ignored_by_git(project_path, None) {
        return is_ignored;
    }

    is_in_local_gitignore(project_path)
}

fn is_ignored_by_git(project_path: &Path, git_config_global: Option<&Path>) -> Option<bool> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(project_path)
        .arg("check-ignore")
        .arg("-q")
        .arg(".tokensave/")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(path) = git_config_global {
        command.env("GIT_CONFIG_GLOBAL", path);
    }

    let status = command.status().ok()?;

    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

fn is_in_local_gitignore(project_path: &Path) -> bool {
    let gitignore = project_path.join(".gitignore");
    match fs::read_to_string(&gitignore) {
        Ok(content) => content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == ".tokensave" || trimmed == ".tokensave/" || trimmed == "/.tokensave"
        }),
        Err(_) => false,
    }
}

/// Appends `.tokensave` to the project's `.gitignore`, creating the file if
/// needed. Ensures the entry starts on its own line (adds a trailing newline
/// to existing content if missing).
///
/// Returns whether the entry is now present, so the caller only reports
/// success when the write actually happened (#288).
pub fn add_to_gitignore(project_path: &Path) -> bool {
    let gitignore = project_path.join(".gitignore");
    let mut content = fs::read_to_string(&gitignore).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(".tokensave\n");
    if let Err(e) = fs::write(&gitignore, content) {
        eprintln!("warning: failed to update .gitignore: {e}");
        return false;
    }
    true
}

/// Appends `.tokensave/` to the repository's local `.git/info/exclude`.
///
/// Unlike `.gitignore`, this file is never committed, so excluding a
/// per-developer index there keeps it out of shared project history.
/// Resolves the exclude path via `git` (so worktrees and
/// custom `$GIT_DIR` layouts are handled), creates the file if missing, and is
/// idempotent — an existing `.tokensave` entry is left untouched.
///
/// Returns whether the entry is now present. Every bail-out path (no locatable
/// git dir, unwritable parent, failed write) returns `false` so the caller
/// never reports an exclusion it did not make (#288); an already-present entry
/// returns `true`, since the desired end state holds.
pub fn add_to_git_info_exclude(project_path: &Path) -> bool {
    let Some(exclude) = git_info_exclude_path(project_path) else {
        eprintln!("warning: could not locate .git/info/exclude");
        return false;
    };
    let content = fs::read_to_string(&exclude).unwrap_or_default();
    if content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == ".tokensave" || trimmed == ".tokensave/" || trimmed == "/.tokensave"
    }) {
        return true;
    }
    if let Some(parent) = exclude.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("warning: failed to create {}: {e}", parent.display());
            return false;
        }
    }
    let mut content = content;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(".tokensave/\n");
    if let Err(e) = fs::write(&exclude, content) {
        eprintln!("warning: failed to update .git/info/exclude: {e}");
        return false;
    }
    true
}

/// The caution shown before indexing a directory that isn't inside a Git
/// working tree (#174), worded for what actually still filters there (#283).
///
/// The scanner reads every `.gitignore` it walks past as a standalone ignore
/// file (`add_custom_ignore_filename` in `scan_files_with_gitignore`), so those
/// patterns apply with or without a `.git` directory. What a non-repo tree
/// loses is everything Git itself supplies: `.git/info/exclude` and the user's
/// global excludes file. The risk that matters — indexing a home directory or
/// another large non-project tree wholesale — is therefore real only when the
/// root has no `.gitignore` to constrain the walk, so say that rather than
/// claiming `.gitignore` filtering is off.
pub fn non_git_scan_warning(project_path: &Path) -> String {
    let root_gitignore = project_path.join(".gitignore");
    let detail = if root_gitignore.is_file() {
        "Its `.gitignore` still applies (tokensave reads ignore files directly), but \
         `.git/info/exclude` and your global excludes do not, so anything only those \
         cover will be indexed."
    } else {
        "There is no `.gitignore` here, so nothing constrains the walk: large \
         non-project trees (e.g. site-packages, virtualenvs, caches) may be indexed \
         and the index can grow very large. Add a `.gitignore` — tokensave honors it \
         without a git repository — or set `exclude` globs in `.tokensave/config.json`."
    };
    format!(
        "\x1b[33mwarning:\x1b[0m '{}' is not inside a git repository.\n  {detail}",
        project_path.display()
    )
}

/// Returns `true` when `project_path` is inside a Git working tree.
///
/// Used to warn before indexing a non-repo tree (e.g. a home directory), where
/// Git's own exclude sources are unavailable and — absent a `.gitignore` —
/// large toolchain trees would be indexed wholesale; see issue #174 and
/// [`non_git_scan_warning`].
pub fn is_inside_git_repo(project_path: &Path) -> bool {
    gix::discover(project_path).is_ok()
}

/// Resolves the absolute path to this repository's `info/exclude` file via
/// `git rev-parse --git-path`. Returns `None` outside a Git repository.
fn git_info_exclude_path(project_path: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("rev-parse")
        .arg("--git-path")
        .arg("info/exclude")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let rel = String::from_utf8(output.stdout).ok()?;
    let rel = rel.trim();
    if rel.is_empty() {
        return None;
    }
    let path = Path::new(rel);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_path.join(path)
    })
}

/// Resolves a CLI path argument to an absolute `PathBuf`.
///
/// If `path` is `Some`, uses that value; otherwise falls back to the current
/// working directory.
pub fn resolve_path(path: Option<String>) -> PathBuf {
    match path {
        Some(p) => absolutize(PathBuf::from(p)),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Anchors a non-absolute CLI path argument to the current working directory.
///
/// Absolute arguments are returned untouched, so a project root a user already
/// stored keeps its exact spelling, symlinks included.
///
/// On Windows a leading-slash path such as `/tmp/project` has a root but no
/// drive, so `is_absolute` is false and it is resolved against the current
/// drive. That matches how Windows itself resolves such a path, and it is
/// required here: leaving it alone would hand a non-absolute path to callers
/// that require one.
///
/// The result is normalized lexically rather than with `canonicalize`, which
/// would fail on a path that does not exist yet and, on Windows, would return a
/// `\\?\` verbatim prefix that then leaks into stored roots and user-facing
/// messages. Normalizing by components also keeps this consistent with the
/// absolute case, where symlinks are likewise left as the user spelled them.
fn absolutize(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    let Ok(cwd) = std::env::current_dir() else {
        return path;
    };
    normalize_lexically(&cwd.join(path))
}

/// Removes `.` and resolves `..` textually, without touching the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Walks from `start` upward looking for a `.tokensave/tokensave.db`.
///
/// Returns the first ancestor directory (inclusive) that contains an
/// initialised `TokenSave` project, or `None` if the filesystem root is
/// reached without finding one.
pub fn discover_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".tokensave/tokensave.db").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Like [`resolve_path`], but when `path` is `None` it walks up from `cwd`
/// to find the nearest initialised `TokenSave` project before falling back to
/// `cwd` itself.
///
/// Used by `serve`, `sync`, and `status`. NOT used by `init` (which must
/// create a fresh project at the target directory).
pub fn resolve_path_with_discovery(path: Option<String>) -> PathBuf {
    if let Some(p) = path {
        absolutize(PathBuf::from(p))
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        discover_project_root(&cwd).unwrap_or(cwd)
    }
}

/// Returns `true` if the path matches any of the configured `include` patterns.
///
/// This is used to allow hidden (dot-prefixed) directories that would
/// otherwise be skipped by the file walker.
pub fn is_included(path: &str, config: &TokenSaveConfig) -> bool {
    let match_opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    for pattern_str in &config.include {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches_with(path, match_opts) {
                return true;
            }
        }
    }

    false
}

/// Returns `true` if a directory should be pruned during scanning.
///
/// Matches `dir/_` against exclude patterns (for `dir/**`-style globs) and
/// also matches `dir` itself (for bare `**/dirname`-style globs).  This
/// ensures that patterns like `**/node_modules` and `**/node_modules/**`
/// both trigger directory pruning in `scan_files_walkdir`.
pub fn is_excluded_dir(dir_path: &str, config: &TokenSaveConfig) -> bool {
    let match_opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    for pattern_str in &config.exclude {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            // Try both the dummy-file probe (catches dir/**) and the bare
            // directory path (catches **/dirname).
            if pattern.matches_with(&format!("{dir_path}/_"), match_opts)
                || pattern.matches_with(dir_path, match_opts)
            {
                return true;
            }
        }
    }

    false
}

/// Returns `true` if the file matches any of the configured exclude patterns.
pub fn is_excluded(file_path: &str, config: &TokenSaveConfig) -> bool {
    let match_opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    for pattern_str in &config.exclude {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches_with(file_path, match_opts) {
                return true;
            }
        }
    }

    false
}

/// A single project-level query-ignore pattern.
///
/// Two flavours are supported:
/// - **Glob** — when the raw pattern contains a `*`, it is compiled with the
///   `glob` crate (the same engine used by the indexing `exclude`/`include`
///   patterns) so segments like `tests/*` or `**/generated/**` work.
/// - **Substring** — any other pattern matches when it appears anywhere in the
///   normalized `file_path` (gitignore-like "name fragment" matching).
///
/// Patterns are matched against node `file_path` values, which are stored
/// relative to the project root and normalized to use `/` separators.
#[derive(Debug, Clone)]
enum IgnoreRule {
    Glob(Pattern),
    Substring(String),
}

impl IgnoreRule {
    fn matches(&self, path: &str) -> bool {
        match self {
            IgnoreRule::Glob(pattern) => pattern.matches_with(
                path,
                glob::MatchOptions {
                    case_sensitive: true,
                    require_literal_separator: false,
                    require_literal_leading_dot: false,
                },
            ),
            IgnoreRule::Substring(needle) => path.contains(needle),
        }
    }
}

/// Project-level set of query-time ignore patterns.
///
/// This is the persistent, implicit counterpart to a per-call path exclusion:
/// once configured in `.tokensave/queryignore`, matching results are dropped
/// from `tokensave_search` and `tokensave_context` without the caller having
/// to pass a filter on every request.
#[derive(Debug, Clone, Default)]
pub struct QueryIgnore {
    rules: Vec<IgnoreRule>,
}

impl QueryIgnore {
    /// Parses query-ignore patterns from raw file contents.
    ///
    /// One pattern per line. Blank lines and lines whose first non-whitespace
    /// character is `#` are ignored. Surrounding whitespace is trimmed. A
    /// pattern containing `*` is treated as a glob; everything else is a
    /// substring match. Invalid globs are silently skipped.
    pub fn parse(contents: &str) -> Self {
        let mut rules = Vec::new();
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let normalized = trimmed.replace('\\', "/");
            if normalized.contains('*') {
                if let Ok(pattern) = Pattern::new(&normalized) {
                    rules.push(IgnoreRule::Glob(pattern));
                }
            } else {
                rules.push(IgnoreRule::Substring(normalized));
            }
        }
        QueryIgnore { rules }
    }

    /// Returns `true` when no patterns are configured (the common case).
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Returns `true` if `file_path` matches any configured ignore pattern.
    /// `file_path` is normalized to `/` separators before matching.
    pub fn is_ignored(&self, file_path: &str) -> bool {
        if self.rules.is_empty() {
            return false;
        }
        let normalized = file_path.replace('\\', "/");
        self.rules.iter().any(|rule| rule.matches(&normalized))
    }
}

/// Loads the project-level query-ignore patterns from
/// `<project_root>/.tokensave/queryignore`.
///
/// Returns an empty [`QueryIgnore`] (matching nothing) when the file is absent
/// or unreadable, so callers can apply it unconditionally with zero behavior
/// change for projects that have not opted in.
///
/// Unlike `config.exclude`, these patterns are applied at QUERY time only and
/// do not affect indexing — a path excluded here is still in the graph, it is
/// merely hidden from `tokensave_search` / `tokensave_context` results. This
/// complements `.gitignore` handling (`config.git_ignore`), which controls
/// what gets indexed in the first place.
pub fn load_query_ignore(project_root: &Path) -> QueryIgnore {
    let path = get_tokensave_dir(project_root).join(QUERYIGNORE_FILENAME);
    match fs::read_to_string(&path) {
        Ok(contents) => QueryIgnore::parse(&contents),
        Err(_) => QueryIgnore::default(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        env_bool_override, is_excluded, is_excluded_dir, is_ignored_by_git, is_included,
        load_query_ignore, QueryIgnore, TokenSaveConfig,
    };
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn test_is_included_matches_glob() {
        let config = TokenSaveConfig {
            include: vec![".github/**".to_string()],
            ..TokenSaveConfig::default()
        };
        assert!(is_included(".github/workflows/ci.yml", &config));
        assert!(is_included(".github/scripts/build.sh", &config));
        assert!(!is_included(".vscode/settings.json", &config));
        assert!(!is_included("src/main.rs", &config));
    }

    #[test]
    fn test_is_included_empty_matches_nothing() {
        let config = TokenSaveConfig::default();
        assert!(!is_included(".github/workflows/ci.yml", &config));
    }

    #[test]
    fn test_include_does_not_override_exclude() {
        let config = TokenSaveConfig {
            include: vec![".config/**".to_string()],
            exclude: vec![".config/secret/**".to_string()],
            ..TokenSaveConfig::default()
        };
        // Included by include glob
        assert!(is_included(".config/secret/key.rs", &config));
        // But also matched by exclude glob
        assert!(is_excluded(".config/secret/key.rs", &config));
    }

    #[test]
    fn test_legacy_config_defaults_source_path_overrides() {
        let mut value = serde_json::to_value(TokenSaveConfig::default()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("source_path_overrides");

        let config: TokenSaveConfig = serde_json::from_value(value).unwrap();
        assert!(config.source_path_overrides.is_empty());
    }

    #[test]
    fn test_default_excludes_nested_node_modules() {
        let config = TokenSaveConfig::default();
        // Top-level node_modules — should be excluded
        assert!(is_excluded("node_modules/express/index.js", &config));
        // Nested node_modules inside a sub-project — must also be excluded
        assert!(is_excluded(
            "projectA/node_modules/express/index.js",
            &config
        ));
        assert!(is_excluded(
            "packages/web/node_modules/react/index.js",
            &config
        ));
    }

    #[test]
    fn test_dir_pruning_pattern_matches_nested_dirs() {
        // scan_files_walkdir checks is_excluded("{dir}/_") for directory pruning.
        // Patterns like **/node_modules/** must match the dummy-file probe.
        let config = TokenSaveConfig::default();
        assert!(is_excluded("node_modules/_", &config));
        assert!(is_excluded("projectA/node_modules/_", &config));
    }

    #[test]
    fn test_is_excluded_dir_bare_pattern() {
        // Users may write "**/node_modules" (no trailing /**).
        // is_excluded_dir should match both bare and /**-suffixed patterns.
        let config = TokenSaveConfig {
            exclude: vec!["**/dist".to_string()],
            ..TokenSaveConfig::default()
        };
        assert!(is_excluded_dir("dist", &config));
        assert!(is_excluded_dir("packages/web/dist", &config));
        // Files inside dist should still be caught by accept_file's is_excluded
        // but dir pruning prevents even walking into the directory.
    }

    #[test]
    fn test_is_in_gitignore_respects_global_excludes_file() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        fs::create_dir(&repo).unwrap();

        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .arg("-q")
            .status()
            .unwrap();

        let excludes = sandbox.path().join("global_ignore");
        fs::write(&excludes, ".tokensave\n").unwrap();

        let git_config = sandbox.path().join("gitconfig");
        let status = Command::new("git")
            .env("GIT_CONFIG_GLOBAL", &git_config)
            .arg("config")
            .arg("--global")
            .arg("core.excludesFile")
            .arg(&excludes)
            .status()
            .unwrap();
        assert!(status.success());

        let ignored = is_ignored_by_git(&repo, Some(&git_config));

        assert_eq!(ignored, Some(true));
    }

    #[test]
    fn test_query_ignore_substring_match() {
        let qi = QueryIgnore::parse("generated\n");
        assert!(qi.is_ignored("src/generated/api.rs"));
        assert!(qi.is_ignored("generated.rs"));
        assert!(!qi.is_ignored("src/main.rs"));
    }

    #[test]
    fn test_query_ignore_glob_match() {
        let qi = QueryIgnore::parse("tests/*\n**/proto/**\n");
        assert!(qi.is_ignored("tests/foo.rs"));
        // `*` does not require a literal separator, so a nested path matches too.
        assert!(qi.is_ignored("tests/sub/bar.rs"));
        assert!(qi.is_ignored("crate/proto/messages.rs"));
        assert!(!qi.is_ignored("src/lib.rs"));
    }

    #[test]
    fn test_query_ignore_skips_comments_and_blanks() {
        let qi = QueryIgnore::parse("# a comment\n\n   \n  vendor  \n");
        assert!(qi.is_ignored("third_party/vendor/lib.rs"));
        assert!(!qi.is_ignored("src/main.rs"));
    }

    #[test]
    fn test_query_ignore_empty_matches_nothing() {
        let qi = QueryIgnore::default();
        assert!(qi.is_empty());
        assert!(!qi.is_ignored("anything/at/all.rs"));
        let parsed = QueryIgnore::parse("# only comments\n\n");
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_query_ignore_normalizes_separators() {
        let qi = QueryIgnore::parse("src/gen\n");
        // file_path with backslashes (Windows-style) should still match.
        assert!(qi.is_ignored("src\\gen\\out.rs"));
    }

    #[test]
    fn test_load_query_ignore_absent_is_empty() {
        let dir = TempDir::new().unwrap();
        let qi = load_query_ignore(dir.path());
        assert!(qi.is_empty());
    }

    #[test]
    fn test_load_query_ignore_reads_file() {
        let dir = TempDir::new().unwrap();
        let ts_dir = dir.path().join(".tokensave");
        fs::create_dir_all(&ts_dir).unwrap();
        fs::write(ts_dir.join("queryignore"), "generated\ntests/*\n").unwrap();

        let qi = load_query_ignore(dir.path());
        assert!(!qi.is_empty());
        assert!(qi.is_ignored("src/generated/x.rs"));
        assert!(qi.is_ignored("tests/foo.rs"));
        assert!(!qi.is_ignored("src/main.rs"));
    }

    #[test]
    fn report_savings_defaults_to_on() {
        // #356 asked for an opt-out, not a change of default.
        assert!(TokenSaveConfig::default().report_savings);
    }

    #[test]
    fn configs_written_before_the_field_existed_keep_reporting() {
        // Serde must not read a missing field as `false` and silently go quiet
        // on every project initialized before #356.
        let json = r#"{"version":1,"root_dir":"/x","exclude":[],"max_file_size":1000,
                       "extract_docstrings":true,"track_call_sites":true}"#;
        let config: TokenSaveConfig = serde_json::from_str(json).unwrap();
        assert!(config.report_savings);
    }

    #[test]
    fn report_savings_round_trips_when_disabled() {
        let config = TokenSaveConfig {
            report_savings: false,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: TokenSaveConfig = serde_json::from_str(&json).unwrap();
        assert!(!back.report_savings);
    }

    #[test]
    fn env_override_respects_falsey_spellings() {
        // Absent → the config value wins, in both directions.
        assert!(env_bool_override("TOKENSAVE_UNSET_TEST_VAR_XYZ", true));
        assert!(!env_bool_override("TOKENSAVE_UNSET_TEST_VAR_XYZ", false));
    }
}
