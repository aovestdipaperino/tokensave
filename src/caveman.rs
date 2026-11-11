//! Opt-in cooperation with an independently installed Caveman (#797).
//!
//! [Caveman](https://github.com/JuliusBrussee/caveman) shortens an agent's
//! prose. `TokenSave` does not bundle, install, configure or run it. When the
//! user opts in (`caveman_compatibility = true` in `~/.tokensave/config.toml`,
//! or `TOKENSAVE_CAVEMAN_COMPATIBILITY`) and a Caveman skill is found, the MCP
//! `initialize` instructions gain [`COOPERATION_INSTRUCTIONS`]. Tool results,
//! schemas and the default instructions are never changed.
//!
//! Everything here is read-only and bounded: fixed candidate paths, at most
//! [`MAX_ARTIFACT_BYTES`] per file and [`MAX_REGISTRY_ENTRIES`] plugin-registry
//! entries, no cache, no background task and no subprocess. An installed skill
//! is evidence of availability only. Session activation is reported solely
//! from Claude's per-session state file for an explicitly named session of a
//! verified Caveman release; it is never inferred from the machine-wide mirror
//! flag or a startup default.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Environment variable that overrides `caveman_compatibility` for one run,
/// parsed like `TOKENSAVE_AUTO_INIT`.
pub const COMPATIBILITY_ENV: &str = "TOKENSAVE_CAVEMAN_COMPATIBILITY";

/// Caveman releases whose session-state layout was checked against the
/// upstream source (`src/hooks/caveman-config.js`).
pub const VERIFIED_RELEASES: &[&str] = &["4.0.1", "4.1.0"];

/// Agents with a skill-discovery adapter.
pub const AGENTS: &[&str] = &[
    "claude", "codex", "cursor", "copilot", "gemini", "opencode", "pi", "omp",
];

/// Upper bound for any file read here.
pub const MAX_ARTIFACT_BYTES: u64 = 64 * 1024;

/// Upper bound for Claude plugin-registry entries considered.
pub const MAX_REGISTRY_ENTRIES: usize = 16;

/// Caveman's own limit for a session state file (`MAX_FLAG_BYTES`).
const MAX_SESSION_BYTES: u64 = 64;

/// Caveman's `SESSION_ID_RE` length limit.
const MAX_SESSION_ID_LEN: usize = 128;

/// Active modes, current and legacy, that Caveman's `canonicalMode` accepts.
const ACTIVE_MODES: &[&str] = &[
    "caveman",
    "ultracave",
    "megacave",
    "commit",
    "review",
    "compress",
    "lite",
    "full",
    "ultra",
    "wenyan",
    "wenyan-lite",
    "wenyan-full",
    "wenyan-ultra",
];

/// Appended to the MCP `initialize` instructions when cooperation is on.
/// It assigns responsibilities and never describes Caveman internals.
pub const COOPERATION_INSTRUCTIONS: &str = " If Caveman is active in this session, it \
    governs only your prose. Keep using targeted tokensave queries for an indexed project \
    (check tokensave_status if readiness is uncertain) and do not repeat broad exploration \
    after a sufficient result. Quote code, identifiers, paths, line numbers, signatures, \
    errors and numbers from tokensave results exactly; never compress or paraphrase them. \
    User instructions take precedence.";

/// Whether a Caveman skill is available to an agent.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Installation {
    /// No supported location holds a Caveman skill.
    Absent,
    /// A skill identifying itself as Caveman was found.
    Installed,
    /// An artifact exists but is malformed, oversized, unreadable or claims
    /// a conflicting identity.
    Partial,
    /// The agent has no discovery adapter.
    Unsupported,
}

/// Whether Caveman is on in one explicitly named session.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    /// The session's state file names an active mode.
    Active,
    /// The session's state file stores `off`.
    Inactive,
    /// No authoritative session state was available.
    Unknown,
}

/// Read-only diagnostics for one agent. Holds paths and fixed messages, never
/// file contents.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Agent identifier, as accepted by `--agent`.
    pub agent: String,
    /// Effective `caveman_compatibility` value.
    pub enabled: bool,
    /// Installation evidence; not an assertion of activation.
    pub installation: Installation,
    /// Session activation, `unknown` unless a session was named.
    pub activation: Activation,
    /// Release from the plugin manifest next to the skill, if any.
    pub version: Option<String>,
    /// Whether `version` is in [`VERIFIED_RELEASES`].
    pub verified_release: bool,
    /// The skill file that was found.
    pub skill: Option<PathBuf>,
    /// What `TokenSave` does with this installation.
    pub capabilities: Vec<&'static str>,
    /// Limits or conflicts worth the user's attention.
    pub warnings: Vec<&'static str>,
}

/// Where to look. Built from the process environment by [`Scope::from_env`];
/// tests build it directly so they never mutate the environment.
#[derive(Debug, Clone)]
pub struct Scope {
    /// The user's home directory.
    pub home: PathBuf,
    /// The project directory; repository-local skills and Caveman
    /// configuration are looked up from here.
    pub project: PathBuf,
    /// An agent home that overrides the default under `home`
    /// (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `XDG_CONFIG_HOME/opencode`).
    pub agent_root: Option<PathBuf>,
    /// Caveman's user configuration files, in Caveman's lookup order.
    pub user_configs: Vec<PathBuf>,
}

impl Scope {
    /// A scope with Caveman's non-Windows default user configuration.
    #[must_use]
    pub fn new(home: &Path, project: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            project: project.to_path_buf(),
            agent_root: None,
            user_configs: vec![default_user_config(home)],
        }
    }

    /// The scope Caveman itself would use for `agent`, from the environment.
    #[must_use]
    pub fn from_env(home: &Path, project: &Path, agent: &str) -> Self {
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let agent_root = match agent {
            "claude" => std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
            "codex" => std::env::var_os("CODEX_HOME").map(PathBuf::from),
            "opencode" => xdg.as_ref().map(|dir| dir.join("opencode")),
            _ => None,
        };
        // Mirrors Caveman's `getConfigDir`: XDG wins everywhere; Windows reads
        // %APPDATA% first and then the POSIX location.
        let user_configs = if let Some(dir) = xdg {
            vec![dir.join("caveman").join("config.json")]
        } else if cfg!(windows) {
            let appdata = std::env::var_os("APPDATA")
                .map_or_else(|| home.join("AppData").join("Roaming"), PathBuf::from);
            vec![
                appdata.join("caveman").join("config.json"),
                default_user_config(home),
            ]
        } else {
            vec![default_user_config(home)]
        };
        Self {
            home: home.to_path_buf(),
            project: project.to_path_buf(),
            agent_root: agent_root.filter(|p| !p.as_os_str().is_empty()),
            user_configs,
        }
    }
}

fn default_user_config(home: &Path) -> PathBuf {
    home.join(".config").join("caveman").join("config.json")
}

/// The effective `caveman_compatibility` value: the environment override when
/// set, otherwise the config file, otherwise `false`.
#[must_use]
pub fn enabled() -> bool {
    crate::config::env_bool_override(
        COMPATIBILITY_ENV,
        crate::user_config::caveman_compatibility_setting(),
    )
}

/// Whether the MCP `initialize` instructions should include
/// [`COOPERATION_INSTRUCTIONS`]. Called once per `initialize`. When the
/// setting is off it returns before touching any agent file.
#[must_use]
pub fn cooperation_enabled(project: Option<&Path>) -> bool {
    if !enabled() {
        return false;
    }
    let Some(home) = crate::agents::home_dir() else {
        return false;
    };
    let project = project.map_or_else(|| home.clone(), Path::to_path_buf);
    AGENTS.iter().any(|agent| {
        inspect(&Scope::from_env(&home, &project, agent), agent, None, true).installation
            == Installation::Installed
    })
}

/// Diagnostics for `agent`, or for every agent in [`AGENTS`].
#[must_use]
pub fn diagnostics(agent: Option<&str>, session_id: Option<&str>, project: &Path) -> Vec<Report> {
    let Some(home) = crate::agents::home_dir() else {
        return Vec::new();
    };
    let enabled = enabled();
    let ids: Vec<&str> = agent.map_or_else(|| AGENTS.to_vec(), |id| vec![id]);
    ids.into_iter()
        .map(|id| {
            inspect(
                &Scope::from_env(&home, project, id),
                id,
                session_id,
                enabled,
            )
        })
        .collect()
}

/// Inspects one agent's Caveman installation without modifying anything.
///
/// `session_id` must come from an explicit caller selection, never from
/// another process's state.
#[must_use]
pub fn inspect(scope: &Scope, agent: &str, session_id: Option<&str>, enabled: bool) -> Report {
    let mut report = Report {
        agent: agent.to_owned(),
        enabled,
        installation: Installation::Absent,
        activation: Activation::Unknown,
        version: None,
        verified_release: false,
        skill: None,
        capabilities: Vec::new(),
        warnings: Vec::new(),
    };
    let (Some(root), Some(project_dir)) = (
        scope
            .agent_root
            .clone()
            .or_else(|| default_agent_root(&scope.home, agent)),
        project_agent_dir(agent),
    ) else {
        report.installation = Installation::Unsupported;
        report
            .warnings
            .push("No Caveman discovery adapter for this agent; tokensave works unchanged.");
        return report;
    };

    let mut candidates = vec![
        skill_file(&scope.project.join(".agents")),
        skill_file(&scope.project.join(project_dir)),
        skill_file(&root),
        skill_file(&scope.home.join(".agents")),
        skill_file(&root.join("caveman")),
    ];
    if agent == "claude" {
        candidates.extend(claude_registry_skills(&root, &mut report));
    }
    find_skill(&candidates, &mut report);
    check_startup_policy(scope, &mut report);
    if agent == "claude" {
        check_claude_settings(&root, &mut report);
    }
    if report.installation == Installation::Installed {
        if root
            .join("agents")
            .join("cavecrew-investigator.md")
            .is_file()
        {
            report.warnings.push(
                "Caveman's cavecrew research agent may overlap tokensave's code-retrieval \
                 guidance; explicit user delegation takes precedence. Nothing was changed.",
            );
        }
        if report.version.is_none() {
            report.warnings.push(
                "Caveman release unknown (no plugin manifest next to the skill); \
                 session state is not interpreted.",
            );
        }
        report.warnings.push(
            "Caveman's proxy or input compression, if configured, can rewrite tool results \
             after tokensave returns them. Exclude tokensave results with Caveman's own controls.",
        );
        if enabled {
            report.capabilities.push("conditional_mcp_instructions");
        }
        if agent == "claude" && report.verified_release {
            report.capabilities.push("session_state_diagnostics");
        }
    }
    if let Some(id) = session_id {
        read_session(&root, agent, id, &mut report);
    }
    report
}

fn skill_file(base: &Path) -> PathBuf {
    base.join("skills").join("caveman").join("SKILL.md")
}

fn default_agent_root(home: &Path, agent: &str) -> Option<PathBuf> {
    Some(match agent {
        "claude" => home.join(".claude"),
        "codex" => home.join(".codex"),
        "cursor" => home.join(".cursor"),
        "copilot" => home.join(".copilot"),
        "gemini" => home.join(".gemini"),
        "opencode" => home.join(".config").join("opencode"),
        "pi" => home.join(".pi").join("agent"),
        "omp" => home.join(".omp").join("agent"),
        _ => return None,
    })
}

fn project_agent_dir(agent: &str) -> Option<&'static str> {
    Some(match agent {
        "claude" => ".claude",
        "codex" => ".codex",
        "cursor" => ".cursor",
        "copilot" => ".copilot",
        "gemini" => ".gemini",
        "opencode" => ".opencode",
        "pi" => ".pi",
        "omp" => ".omp",
        _ => return None,
    })
}

/// Skill paths named by Claude's plugin registry. Caches are never scanned,
/// so a stale cached version is never picked up by accident.
fn claude_registry_skills(root: &Path, report: &mut Report) -> Vec<PathBuf> {
    let registry = root.join("plugins").join("installed_plugins.json");
    let raw = match read_artifact(&registry) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(_) => {
            report
                .warnings
                .push("Claude plugin registry unreadable; plugin discovery incomplete.");
            return Vec::new();
        }
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        report
            .warnings
            .push("Claude plugin registry malformed; plugin discovery incomplete.");
        return Vec::new();
    };
    value["plugins"]["caveman@caveman"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .take(MAX_REGISTRY_ENTRIES)
                .filter_map(|entry| entry["installPath"].as_str())
                .map(|path| skill_file(Path::new(path)))
                .collect()
        })
        .unwrap_or_default()
}

/// Takes the first readable skill that identifies as Caveman.
fn find_skill(candidates: &[PathBuf], report: &mut Report) {
    for path in candidates {
        let text = match read_artifact(path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(_) => {
                report.installation = Installation::Partial;
                report
                    .warnings
                    .push("A Caveman skill is unreadable, not UTF-8 or larger than 64 KiB.");
                continue;
            }
        };
        if !text.lines().any(|line| line.trim() == "name: caveman") {
            report.installation = Installation::Partial;
            report
                .warnings
                .push("A Caveman skill has no `name: caveman` identity.");
            continue;
        }
        report.skill = Some(path.clone());
        report.installation = Installation::Installed;
        check_manifest(path, report);
        return;
    }
}

/// Reads the plugin manifest of a linked or plugin-installed skill. A copied
/// skill usually has none, and then the release stays unknown.
fn check_manifest(skill: &Path, report: &mut Report) {
    let resolved = skill.canonicalize().unwrap_or_else(|_| skill.to_path_buf());
    let Some(base) = resolved
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return;
    };
    let manifest = base.join(".claude-plugin").join("plugin.json");
    let raw = match read_artifact(&manifest) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(_) => {
            report.installation = Installation::Partial;
            report
                .warnings
                .push("Caveman plugin manifest could not be read safely.");
            return;
        }
    };
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) if value["name"] == "caveman" => {
            report.version = value["version"]
                .as_str()
                .filter(|v| v.len() <= 64)
                .map(str::to_owned);
            report.verified_release = report
                .version
                .as_deref()
                .is_some_and(|v| VERIFIED_RELEASES.contains(&v));
            if !report.verified_release {
                report.warnings.push(
                    "Caveman release not verified by this tokensave version; \
                     session state is not interpreted.",
                );
            }
        }
        _ => {
            report.installation = Installation::Partial;
            report
                .warnings
                .push("Caveman plugin manifest is malformed or names another plugin.");
        }
    }
}

/// Reports Caveman's startup policy. A default never proves that the current
/// session is active, so this only adds warnings.
fn check_startup_policy(scope: &Scope, report: &mut Report) {
    // Caveman walks up from the working directory; the first file that names
    // `defaultMode` wins, then the user configuration.
    let repo = scope.project.ancestors().flat_map(|dir| {
        [
            dir.join(".caveman").join("config.json"),
            dir.join(".caveman.json"),
        ]
    });
    for path in repo.chain(scope.user_configs.iter().cloned()) {
        let raw = match read_artifact(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(_) => {
                report
                    .warnings
                    .push("A Caveman configuration file is unreadable or larger than 64 KiB.");
                continue;
            }
        };
        let value = crate::agents::fs::parse_jsonc(raw.trim_start_matches('\u{feff}'));
        let Some(mode) = value.get("defaultMode") else {
            continue;
        };
        match mode.as_str().map(str::to_ascii_lowercase).as_deref() {
            Some("off" | "manual") => report.warnings.push(
                "Caveman's startup default is off or manual; it says nothing about the \
                 current session.",
            ),
            Some(m) if ACTIVE_MODES.contains(&m) => {}
            _ => report
                .warnings
                .push("Caveman's startup default is not a mode this tokensave version recognizes."),
        }
        return;
    }
}

fn check_claude_settings(root: &Path, report: &mut Report) {
    if let Ok(raw) = read_artifact(&root.join("settings.json")) {
        let settings = crate::agents::fs::parse_jsonc(&raw);
        if settings["enabledPlugins"]["caveman@caveman"] == false {
            report.warnings.push(
                "The Caveman plugin is disabled in Claude settings; a skill on disk is not \
                 activation.",
            );
        }
    }
}

/// Reads `<claude root>/.caveman-sessions/<id>.mode`, the per-session state
/// Caveman writes for Claude Code.
fn read_session(root: &Path, agent: &str, id: &str, report: &mut Report) {
    if agent != "claude"
        || report.installation != Installation::Installed
        || !report.verified_release
    {
        report.warnings.push(
            "Session diagnostics need an installed Caveman release verified by this \
             tokensave version, for Claude Code.",
        );
        return;
    }
    if id.is_empty()
        || id.len() > MAX_SESSION_ID_LEN
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        report
            .warnings
            .push("Invalid session id; no session file was read.");
        return;
    }
    let path = root.join(".caveman-sessions").join(format!("{id}.mode"));
    // Like Caveman's `readFlag`: no symlinks, regular files, tiny.
    let valid = std::fs::symlink_metadata(&path)
        .is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_SESSION_BYTES);
    let mode = if valid {
        read_artifact(&path).ok()
    } else {
        None
    };
    match mode.map(|m| m.trim().to_ascii_lowercase()).as_deref() {
        Some("off") => report.activation = Activation::Inactive,
        Some(m) if ACTIVE_MODES.contains(&m) => report.activation = Activation::Active,
        Some(_) => report
            .warnings
            .push("Session state names no known mode; activation unknown."),
        None => report
            .warnings
            .push("Session state absent or unreadable; activation unknown."),
    }
}

/// Reads a regular file of at most [`MAX_ARTIFACT_BYTES`]. The bound holds
/// even if the file grows after the size check.
fn read_artifact(path: &Path) -> std::io::Result<String> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let mut text = String::new();
    File::open(path)?
        .take(MAX_ARTIFACT_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(std::io::Error::other("file too large"));
    }
    Ok(text)
}
