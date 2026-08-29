// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

use crate::agents::integrations::*;
use crate::agents::traits::AgentIntegration;
use crate::errors::TokenSaveError;
use std::path::Path;

/// Returns the agent matching `id`, or an error if unknown.
pub fn get_integration(id: &str) -> crate::errors::Result<Box<dyn AgentIntegration>> {
    match id {
        "claude" => Ok(Box::new(ClaudeIntegration)),
        "opencode" => Ok(Box::new(OpenCodeIntegration)),
        "codex" => Ok(Box::new(CodexIntegration)),
        "gemini" => Ok(Box::new(GeminiIntegration)),
        "qwen" => Ok(Box::new(QwenIntegration)),
        "copilot" => Ok(Box::new(CopilotIntegration)),
        "cursor" => Ok(Box::new(CursorIntegration)),
        "droid" => Ok(Box::new(DroidIntegration)),
        "zed" => Ok(Box::new(ZedIntegration)),
        "cline" => Ok(Box::new(ClineIntegration)),
        "roo-code" => Ok(Box::new(RooCodeIntegration)),
        "antigravity" => Ok(Box::new(AntigravityIntegration)),
        "kilo" => Ok(Box::new(KiloIntegration)),
        "kiro" => Ok(Box::new(KiroIntegration)),
        "kimi" => Ok(Box::new(KimiIntegration)),
        "vibe" => Ok(Box::new(VibeIntegration)),
        "grok" => Ok(Box::new(GrokIntegration)),
        "omp" => Ok(Box::new(OmpIntegration)),
        "pi" => Ok(Box::new(PiIntegration)),
        "plank" => Ok(Box::new(PlankIntegration)),
        "auggie" => Ok(Box::new(AugmentIntegration)),
        _ => Err(TokenSaveError::Config {
            message: format!(
                "unknown agent: \"{id}\". Available agents: {}",
                available_integrations().join(", ")
            ),
        }),
    }
}

/// Returns all registered agents.
pub fn all_integrations() -> Vec<Box<dyn AgentIntegration>> {
    vec![
        Box::new(ClaudeIntegration),
        Box::new(OpenCodeIntegration),
        Box::new(CodexIntegration),
        Box::new(GeminiIntegration),
        Box::new(QwenIntegration),
        Box::new(CopilotIntegration),
        Box::new(CursorIntegration),
        Box::new(DroidIntegration),
        Box::new(ZedIntegration),
        Box::new(ClineIntegration),
        Box::new(RooCodeIntegration),
        Box::new(AntigravityIntegration),
        Box::new(KiloIntegration),
        Box::new(KiroIntegration),
        Box::new(KimiIntegration),
        Box::new(VibeIntegration),
        Box::new(GrokIntegration),
        Box::new(OmpIntegration),
        Box::new(PiIntegration),
        Box::new(PlankIntegration),
        Box::new(AugmentIntegration),
    ]
}

/// Returns the CLI identifiers of all registered agents (for help text).
pub fn available_integrations() -> Vec<&'static str> {
    vec![
        "claude",
        "opencode",
        "codex",
        "gemini",
        "qwen",
        "copilot",
        "cursor",
        "droid",
        "zed",
        "cline",
        "roo-code",
        "antigravity",
        "kilo",
        "kiro",
        "kimi",
        "vibe",
        "grok",
        "omp",
        "pi",
        "plank",
        "auggie",
    ]
}

/// Returns agent IDs that have tokensave configured under `home` but are
/// absent from `current`. Pure — does no I/O on the config file.
pub fn detect_missing_installed_agents(home: &Path, current: &[String]) -> Vec<String> {
    let mut additions = Vec::new();
    for ag in all_integrations() {
        let id = ag.id().to_string();
        if ag.has_tokensave(home) && !current.contains(&id) {
            additions.push(id);
        }
    }
    additions
}

/// Backfill `installed_agents` for users upgrading from older versions.
///
/// Always scans every agent and adds any that have tokensave configured
/// (e.g. an `~/.claude.json` MCP server entry) but are absent from
/// `installed_agents`. Without the additive scan, a user who installed
/// agent A first and agent B later would have only A in the list, so
/// `tokensave reinstall` would silently skip B and its tool permissions
/// would never be refreshed when new tools ship.
pub fn migrate_installed_agents(home: &Path, config: &mut crate::user_config::UserConfig) {
    let additions = detect_missing_installed_agents(home, &config.installed_agents);
    if additions.is_empty() {
        return;
    }
    config.installed_agents.extend(additions);
    config.save();
}

/// Interactively pick which agents to install/uninstall.
///
/// - 0 detected agents → returns an error.
/// - 1 detected and not already installed → returns it directly (no prompt).
/// - Otherwise → asks a Y/n question for each detected agent.
///
/// Returns `(to_install, to_uninstall)`.
pub fn pick_integrations_interactive(
    home: &Path,
    installed: &[String],
) -> crate::errors::Result<(Vec<String>, Vec<String>)> {
    let detected: Vec<Box<dyn AgentIntegration>> = all_integrations()
        .into_iter()
        .filter(|ag| ag.is_detected(home))
        .collect();

    if detected.is_empty() {
        return Err(TokenSaveError::Config {
            message: "No supported agents detected on this system".to_string(),
        });
    }

    // Fast path: exactly one detected agent and it isn't installed yet.
    if detected.len() == 1 && !installed.contains(&detected[0].id().to_string()) {
        let id = detected[0].id().to_string();
        return Ok((vec![id], vec![]));
    }

    let mut to_install = Vec::new();
    let mut to_uninstall = Vec::new();

    for ag in &detected {
        let id = ag.id().to_string();
        let already = installed.contains(&id);
        if already {
            eprint!("Keep tokensave for {}? [Y/n] ", ag.name());
        } else {
            eprint!("Install tokensave for {}? [Y/n] ", ag.name());
        }

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to read input: {e}"),
            })?;
        let answer = input.trim().to_lowercase();
        let yes = answer.is_empty() || answer == "y" || answer == "yes";

        if yes && !already {
            to_install.push(id);
        } else if !yes && already {
            to_uninstall.push(id);
        }
    }

    Ok((to_install, to_uninstall))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod migrate_tests {
    use super::*;
    use std::fs;

    /// Writes a minimal `~/.claude.json` so `ClaudeIntegration::has_tokensave`
    /// returns true for the given fake home.
    fn install_claude_marker(home: &Path) {
        let claude_json = home.join(".claude.json");
        fs::write(
            &claude_json,
            r#"{"mcpServers":{"tokensave":{"command":"tokensave","args":["serve"]}}}"#,
        )
        .unwrap();
    }

    /// Regression test for the bug where `tokensave reinstall` skipped Claude
    /// when another agent (e.g. copilot) was already in `installed_agents`.
    /// `migrate_installed_agents` previously returned early as soon as the
    /// list was non-empty, so Claude never got tracked and its tool perms
    /// never refreshed.
    #[test]
    fn detects_claude_when_another_agent_already_tracked() {
        let dir = tempfile::tempdir().unwrap();
        install_claude_marker(dir.path());

        let current = vec!["copilot".to_string()];
        let additions = detect_missing_installed_agents(dir.path(), &current);

        assert!(
            additions.iter().any(|id| id == "claude"),
            "claude must be detected even when copilot is already in the list, got {additions:?}"
        );
    }

    #[test]
    fn detects_claude_when_list_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        install_claude_marker(dir.path());

        let additions = detect_missing_installed_agents(dir.path(), &[]);

        assert!(additions.iter().any(|id| id == "claude"));
    }

    #[test]
    fn no_additions_when_claude_already_tracked() {
        let dir = tempfile::tempdir().unwrap();
        install_claude_marker(dir.path());

        let current = vec!["claude".to_string()];
        let additions = detect_missing_installed_agents(dir.path(), &current);

        assert!(
            !additions.contains(&"claude".to_string()),
            "claude is already tracked; must not be re-added, got {additions:?}"
        );
    }

    #[test]
    fn empty_home_yields_no_additions() {
        let dir = tempfile::tempdir().unwrap();
        let additions = detect_missing_installed_agents(dir.path(), &[]);
        assert!(
            additions.is_empty(),
            "no agent files in home → no additions, got {additions:?}"
        );
    }
}
