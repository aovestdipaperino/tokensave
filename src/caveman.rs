//! Read-only, bounded discovery of independently managed Caveman skills.
//!
//! Installation is evidence of availability, never session activation. The
//! only session state read is Claude's documented, explicitly identified file.
//! No third-party executable is run, and MCP results never pass through here.
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Instructions concern responsibilities, not Caveman's implementation details.
pub const COOPERATION_INSTRUCTIONS: &str = " If Caveman is active in this session, \
    use targeted TokenSave semantic queries for an indexed project when suitable; \
    if readiness is uncertain, check tokensave_status first, \
    and avoid repeating broad exploration after a sufficient result. Caveman owns \
    its supported prose style and efficiency features. Preserve code, identifiers, \
    paths, signatures, errors, numbers, and semantic relationships exactly; never compress \
    or paraphrase structured TokenSave results. These retrieval instructions do \
    not select models, route providers, configure proxies, or govern unrelated \
    delegation. User instructions take precedence.";

/// Availability and activation are separate dimensions.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Installation {
    /// None of the supported locations contains evidence.
    Absent,
    /// A recognisable skill exists; activation is separately reported.
    Installed,
    /// An incomplete, malformed, oversized or unreadable artifact exists.
    Partial,
    /// Metadata explicitly names an unverified release or conflicting identity.
    Unsupported,
}

/// No global flag or default policy can prove current-session activation.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    /// An explicitly identified, supported session is on.
    Active,
    /// An explicitly identified, supported session stores off.
    Inactive,
    /// The host did not expose an authoritative session identity/state.
    Unknown,
}

/// Serializable diagnostics contain paths and fixed messages, never config bodies.
#[derive(Debug, Serialize)]
pub struct Report {
    /// `TokenSave` agent identifier.
    pub agent: String,
    /// Whether automatic cooperation is enabled.
    pub enabled: bool,
    /// Installation evidence; not an assertion of session activation.
    pub installation: Installation,
    /// Session-specific activation evidence.
    pub activation: Activation,
    /// Release metadata when bundled with a skill.
    pub version: Option<String>,
    /// Located skill file.
    pub skill: Option<PathBuf>,
    /// Supported cooperation surface.
    pub capabilities: Vec<&'static str>,
    /// Limits or conflicts requiring attention.
    pub warnings: Vec<&'static str>,
}

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024;

/// Evaluated once during MCP initialization, never on retrieval's hot path.
#[must_use]
pub fn cooperation_enabled(project: Option<&Path>) -> bool {
    let enabled = enabled();
    if !enabled {
        return false;
    }
    diagnostics_at(None, None, project, enabled)
        .iter()
        .any(|r| r.installation == Installation::Installed)
}

/// Read-only diagnostics for the current project and selected/all known agents.
#[must_use]
pub fn diagnostics(agent: Option<&str>, session_id: Option<&str>) -> Vec<Report> {
    diagnostics_at(agent, session_id, None, enabled())
}

fn diagnostics_at(
    agent: Option<&str>,
    session_id: Option<&str>,
    project: Option<&Path>,
    enabled: bool,
) -> Vec<Report> {
    let Some(home) = crate::agents::home_dir() else {
        return vec![];
    };
    let project = project.map_or_else(
        || std::env::current_dir().unwrap_or_else(|_| home.clone()),
        Path::to_path_buf,
    );
    let ids = agent.map_or_else(
        || {
            vec![
                "claude", "codex", "cursor", "copilot", "gemini", "opencode", "pi", "omp",
            ]
        },
        |id| vec![id],
    );
    ids.into_iter()
        .map(|id| {
            let root = match id {
                "claude" => std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
                "codex" => std::env::var_os("CODEX_HOME").map(PathBuf::from),
                "opencode" => {
                    std::env::var_os("XDG_CONFIG_HOME").map(|p| PathBuf::from(p).join("opencode"))
                }
                _ => None,
            };
            inspect(&home, &project, id, root.as_deref(), session_id, enabled)
        })
        .collect()
}

/// Existing user preference plus a per-run, TokenSave-owned opt-out.
#[must_use]
pub fn enabled() -> bool {
    crate::config::env_bool_override(
        "TOKENSAVE_CAVEMAN_COMPATIBILITY",
        crate::user_config::UserConfig::load().caveman_compatibility,
    )
}

/// Inspect known skill locations without modifying files or executing Caveman.
/// `agent_root` permits caller-resolved custom agent homes. `session_id` must
/// come from an explicit caller selection, never another process's mirror flag.
#[must_use]
pub fn inspect(
    home: &Path,
    project: &Path,
    agent: &str,
    agent_root: Option<&Path>,
    session_id: Option<&str>,
    enabled: bool,
) -> Report {
    let mut report = Report {
        agent: agent.to_owned(),
        enabled,
        installation: Installation::Absent,
        activation: Activation::Unknown,
        version: None,
        skill: None,
        capabilities: vec![],
        warnings: vec![],
    };
    let Some(root) = agent_root
        .map(Path::to_path_buf)
        .or_else(|| root_for(home, agent))
    else {
        report.installation = Installation::Unsupported;
        report.warnings.push("No verified automatic discovery adapter for this agent; standalone TokenSave remains available.");
        return report;
    };
    let project_dir = match agent {
        "claude" => ".claude",
        "codex" => ".codex",
        "cursor" => ".cursor",
        "copilot" => ".copilot",
        "gemini" => ".gemini",
        "opencode" => ".opencode",
        "pi" => ".pi",
        "omp" => ".omp",
        _ => {
            report.installation = Installation::Unsupported;
            report
                .warnings
                .push("No verified automatic discovery adapter for this agent.");
            return report;
        }
    };
    let mut candidates = vec![
        project
            .join(".agents")
            .join("skills")
            .join("caveman")
            .join("SKILL.md"),
        project
            .join(project_dir)
            .join("skills")
            .join("caveman")
            .join("SKILL.md"),
        root.join("skills").join("caveman").join("SKILL.md"),
        home.join(".agents")
            .join("skills")
            .join("caveman")
            .join("SKILL.md"),
        root.join("caveman")
            .join("skills")
            .join("caveman")
            .join("SKILL.md"),
    ];
    // Claude's plugin registry is authoritative installation evidence, not
    // activation. Do not scan caches or pick an arbitrary cached version.
    if agent == "claude" {
        let registry = root.join("plugins").join("installed_plugins.json");
        match read_artifact(&registry) {
            Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(value) => {
                    if let Some(entries) = value["plugins"]["caveman@caveman"].as_array() {
                        for entry in entries.iter().take(16) {
                            if let Some(path) = entry["installPath"].as_str() {
                                candidates.push(
                                    PathBuf::from(path)
                                        .join("skills")
                                        .join("caveman")
                                        .join("SKILL.md"),
                                );
                            }
                        }
                        if !entries.is_empty() {
                            report.installation = Installation::Partial;
                            report.warnings.push("Plugin registry found; cached installation may be stale or disabled. Session activation remains separate.");
                        }
                    }
                }
                Err(_) => report
                    .warnings
                    .push("Claude plugin registry malformed; plugin discovery incomplete."),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => report
                .warnings
                .push("Claude plugin registry unreadable; plugin discovery incomplete."),
        }
    }
    for path in candidates {
        let text = match read_artifact(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                report.installation = Installation::Partial;
                report
                    .warnings
                    .push("Caveman skill unreadable, invalid UTF-8 or larger than 64 KiB.");
                continue;
            }
        };
        report.skill = Some(path.clone());
        if !text.lines().any(|line| line.trim() == "name: caveman") {
            report.installation = Installation::Partial;
            report
                .warnings
                .push("Caveman skill identity is missing or malformed.");
            continue;
        }
        report.installation = Installation::Installed;
        // Linked skills may retain the upstream plugin manifest. Copied skills
        // commonly have none: report unknown rather than inventing a version.
        let resolved = path.canonicalize().unwrap_or(path);
        if let Some(base) = resolved
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
        {
            let manifest = base.join(".claude-plugin").join("plugin.json");
            match read_artifact(&manifest) {
                Ok(raw) => {
                    match serde_json::from_str::<serde_json::Value>(&raw) {
                        Ok(value) if value["name"] == "caveman" => {
                            report.version = value["version"]
                                .as_str()
                                .filter(|v| v.len() <= 64)
                                .map(str::to_owned);
                            if report.version.as_deref() != Some("4.0.1") {
                                report.installation = Installation::Unsupported;
                                report.warnings.push("Unverified Caveman release; session-state interpretation disabled.");
                            }
                        }
                        _ => {
                            report.installation = Installation::Partial;
                            report.warnings.push("Caveman plugin metadata is malformed or has a conflicting identity.");
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    report.installation = Installation::Partial;
                    report
                        .warnings
                        .push("Caveman plugin metadata could not be read safely.");
                }
            }
        }
        break;
    }
    // A startup default is diagnostic context, never proof of activation.
    for config in [
        project.join(".caveman").join("config.json"),
        project.join(".caveman.json"),
        home.join(".config").join("caveman").join("config.json"),
    ] {
        match read_artifact(&config) {
            Ok(raw) => {
                let value = crate::agents::fs::parse_jsonc(raw.trim_start_matches('\u{feff}'));
                if value.as_object().is_none_or(serde_json::Map::is_empty) && raw.trim() != "{}" {
                    report.warnings.push("Caveman configuration could not be interpreted; activation remains unknown. JSONC and UTF-8 are supported; UTF-16 settings are not interpreted.");
                    if report.installation == Installation::Absent {
                        report.installation = Installation::Partial;
                    }
                } else if let Some(mode) = value.get("defaultMode") {
                    if mode == "off" || mode == "manual" {
                        report.warnings.push("Caveman startup policy is off/manual; it does not establish the current session's mode.");
                    } else if !mode.as_str().is_some_and(|m| {
                        matches!(
                            m.to_ascii_lowercase().as_str(),
                            "caveman"
                                | "ultracave"
                                | "megacave"
                                | "commit"
                                | "review"
                                | "compress"
                                | "lite"
                                | "full"
                                | "ultra"
                                | "wenyan"
                                | "wenyan-lite"
                                | "wenyan-full"
                                | "wenyan-ultra"
                        )
                    }) {
                        report.warnings.push(
                            "Unrecognised Caveman startup mode; configuration requires review.",
                        );
                        report.installation = Installation::Unsupported;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                report.warnings.push(
                    "Caveman configuration unreadable or oversized; no configuration changed.",
                );
                if report.installation == Installation::Absent {
                    report.installation = Installation::Partial;
                }
            }
        }
    }
    if agent == "claude" {
        if let Ok(raw) = read_artifact(&root.join("settings.json")) {
            let settings = crate::agents::fs::parse_jsonc(&raw);
            if settings["enabledPlugins"]["caveman@caveman"] == false {
                report.warnings.push("Caveman plugin explicitly disabled in Claude settings; skill availability is not activation.");
            }
        }
    }
    if report.installation != Installation::Absent {
        if root
            .join("agents")
            .join("cavecrew-investigator.md")
            .is_file()
        {
            report.warnings.push("Cavecrew research-agent instructions may overlap TokenSave's code-retrieval guardrail. Prefer targeted semantic queries for suitable code research; explicit user delegation takes precedence. No instructions changed.");
        }
        if report.version.is_none() {
            report.warnings.push("Caveman release unknown; skill presence does not establish activation or proxy capabilities.");
        }
        report.warnings.push("Proxy input compression can transform tool results. No verified per-TokenSave exclusion interface; byte preservation beyond the MCP boundary cannot be guaranteed. Keep TokenSave results out of lossy compression using Caveman's own controls.");
        if enabled && report.installation == Installation::Installed {
            report.capabilities.push("conditional_mcp_instructions");
        }
    }
    if let Some(id) = session_id {
        if agent != "claude"
            || report.installation != Installation::Installed
            || report.version.as_deref() != Some("4.0.1")
        {
            report.warnings.push(
                "Session-state diagnostics require a verified Caveman 4.0.1 Claude installation.",
            );
        } else if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            report
                .warnings
                .push("Invalid session identity; no session file read.");
        } else {
            let path = root.join(".caveman-sessions").join(format!("{id}.mode"));
            let valid_file = std::fs::symlink_metadata(&path)
                .is_ok_and(|meta| meta.is_file() && meta.len() <= 64);
            let mode = if valid_file {
                read_artifact(&path)
            } else {
                Err(std::io::Error::other("invalid session file"))
            };
            match mode {
                Ok(mode) => match mode.trim().to_ascii_lowercase().as_str() {
                    "off" => report.activation = Activation::Inactive,
                    "caveman" | "ultracave" | "megacave" | "commit" | "review" | "compress"
                    | "lite" | "full" | "ultra" | "wenyan" | "wenyan-lite" | "wenyan-full"
                    | "wenyan-ultra" => report.activation = Activation::Active,
                    _ => report
                        .warnings
                        .push("Malformed session mode; activation unknown."),
                },
                Err(_) => report
                    .warnings
                    .push("Session state absent or unreadable; activation unknown."),
            }
        }
    }
    report
}

// Bounded allocation even if a file grows after open. Handles close on return.
fn read_artifact(path: &Path) -> std::io::Result<String> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let mut text = String::new();
    File::open(path)?
        .take(MAX_ARTIFACT_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(std::io::Error::other("artifact too large"));
    }
    Ok(text)
}

fn root_for(home: &Path, agent: &str) -> Option<PathBuf> {
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
