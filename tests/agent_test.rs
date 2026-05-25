use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::*;

// ---------------------------------------------------------------------------
// 1. Registry tests
// ---------------------------------------------------------------------------

#[test]
fn test_get_all_integrations() {
    let all = all_integrations();
    assert_eq!(all.len(), 14);
}

#[test]
fn test_available_integrations() {
    let ids = available_integrations();
    assert!(ids.contains(&"claude"));
    assert!(ids.contains(&"copilot"));
    assert!(ids.contains(&"codex"));
    assert!(ids.contains(&"gemini"));
    assert!(ids.contains(&"opencode"));
    assert!(ids.contains(&"cursor"));
    assert!(ids.contains(&"zed"));
    assert!(ids.contains(&"cline"));
    assert!(ids.contains(&"roo-code"));
    assert!(ids.contains(&"antigravity"));
    assert!(ids.contains(&"kilo"));
    assert!(ids.contains(&"kiro"));
    assert!(ids.contains(&"kimi"));
    assert!(ids.contains(&"vibe"));
    assert_eq!(ids.len(), 14);
}

#[test]
fn test_get_integration_valid() {
    for id in &[
        "claude",
        "opencode",
        "codex",
        "gemini",
        "copilot",
        "cursor",
        "zed",
        "cline",
        "roo-code",
        "antigravity",
        "kilo",
        "kiro",
        "kimi",
        "vibe",
    ] {
        let agent = get_integration(id).unwrap();
        assert_eq!(agent.id(), *id);
    }
}

#[test]
fn test_get_integration_invalid() {
    assert!(get_integration("nonexistent").is_err());
    assert!(get_integration("").is_err());
    assert!(get_integration("CLAUDE").is_err()); // case-sensitive
}

// ---------------------------------------------------------------------------
// 2. Agent trait tests (name/id)
// ---------------------------------------------------------------------------

#[test]
fn test_agent_names_and_ids() {
    for agent in all_integrations() {
        assert!(!agent.name().is_empty(), "agent name should not be empty");
        assert!(!agent.id().is_empty(), "agent id should not be empty");
    }
}

#[test]
fn test_agent_names_are_human_readable() {
    // Names should have at least one space or capital letter (human-readable, not slug)
    let expected_names: Vec<(&str, &str)> = vec![
        ("claude", "Claude Code"),
        ("copilot", "GitHub Copilot"),
        ("codex", "Codex CLI"),
        ("gemini", "Gemini CLI"),
        ("opencode", "OpenCode"),
        ("cursor", "Cursor"),
        ("zed", "Zed"),
        ("cline", "Cline"),
        ("roo-code", "Roo Code"),
        ("antigravity", "Antigravity"),
        ("kilo", "Kilo CLI"),
        ("kiro", "Kiro"),
        ("kimi", "Kimi CLI"),
        ("vibe", "Mistral Vibe"),
    ];
    for (id, expected_name) in expected_names {
        let agent = get_integration(id).unwrap();
        assert_eq!(agent.name(), expected_name, "name mismatch for agent {id}");
    }
}

// ---------------------------------------------------------------------------
// 3. Install / config creation tests (with tempdir)
// ---------------------------------------------------------------------------

fn make_install_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: expected_tool_perms(),
    }
}

#[test]
fn test_claude_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Check ~/.claude.json exists and has mcpServers.tokensave
    let claude_json = home.join(".claude.json");
    assert!(
        claude_json.exists(),
        "~/.claude.json should exist after install"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert!(
        content.get("mcpServers").is_some(),
        "mcpServers key should exist"
    );
    assert!(
        content["mcpServers"]["tokensave"].is_object(),
        "mcpServers.tokensave should be an object"
    );
    // Verify args contain "serve"
    let args = content["mcpServers"]["tokensave"]["args"]
        .as_array()
        .unwrap();
    assert!(args.iter().any(|v| v.as_str() == Some("serve")));

    // Check ~/.claude/settings.json exists with hook and permissions
    let settings_path = home.join(".claude/settings.json");
    assert!(
        settings_path.exists(),
        "settings.json should exist after install"
    );
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    // Check hook
    assert!(
        settings["hooks"]["PreToolUse"].is_array(),
        "PreToolUse hook should be an array"
    );
    // Check permissions
    assert!(
        settings["permissions"]["allow"].is_array(),
        "permissions.allow should be an array"
    );

    // Check CLAUDE.md exists with tokensave rules
    let claude_md = home.join(".claude/CLAUDE.md");
    assert!(claude_md.exists(), "CLAUDE.md should exist after install");
    let md_content = std::fs::read_to_string(&claude_md).unwrap();
    assert!(
        md_content.contains("tokensave"),
        "CLAUDE.md should mention tokensave"
    );
}

#[test]
fn test_gemini_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();

    // Check ~/.gemini/settings.json
    let settings_path = home.join(".gemini/settings.json");
    assert!(
        settings_path.exists(),
        "settings.json should exist after install"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(
        content["mcpServers"]["tokensave"].is_object(),
        "mcpServers.tokensave should exist"
    );
    // Verify trust flag
    assert_eq!(
        content["mcpServers"]["tokensave"]["trust"],
        serde_json::json!(true),
        "gemini should have trust: true"
    );

    // Check GEMINI.md
    let gemini_md = home.join(".gemini/GEMINI.md");
    assert!(gemini_md.exists(), "GEMINI.md should exist after install");
    let md_content = std::fs::read_to_string(&gemini_md).unwrap();
    assert!(md_content.contains("tokensave"));
}

#[test]
fn test_codex_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();

    // Check ~/.codex/config.toml
    let config_path = home.join(".codex/config.toml");
    assert!(
        config_path.exists(),
        "config.toml should exist after install"
    );
    // Verify the file contains the expected content as text (the TOML output from
    // toml::to_string_pretty uses dotted headers which may not round-trip through
    // toml::Value::parse in all crate versions)
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("[mcp_servers.tokensave]"),
        "config.toml should contain [mcp_servers.tokensave]"
    );
    assert!(
        content.contains("\"serve\""),
        "config.toml should contain \"serve\" in args"
    );

    // Check AGENTS.md
    let agents_md = home.join(".codex/AGENTS.md");
    assert!(agents_md.exists(), "AGENTS.md should exist after install");
    let md_content = std::fs::read_to_string(&agents_md).unwrap();
    assert!(md_content.contains("tokensave"));
}

#[test]
fn test_kimi_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    KimiIntegration.install(&ctx).unwrap();

    let mcp_path = home.join(".kimi/mcp.json");
    assert!(mcp_path.exists(), "mcp.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(
        content["mcpServers"]["tokensave"].is_object(),
        "mcpServers.tokensave should be an object"
    );
    let args = content["mcpServers"]["tokensave"]["args"]
        .as_array()
        .unwrap();
    assert!(args.iter().any(|v| v.as_str() == Some("serve")));

    let agents_md = home.join(".kimi/AGENTS.md");
    assert!(agents_md.exists(), "AGENTS.md should exist after install");
    let md_content = std::fs::read_to_string(&agents_md).unwrap();
    assert!(md_content.contains("tokensave"));
}

#[test]
fn test_kimi_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    KimiIntegration.install(&ctx).unwrap();
    let mcp_path = home.join(".kimi/mcp.json");
    assert!(mcp_path.exists());

    KimiIntegration.uninstall(&ctx).unwrap();

    assert!(
        !mcp_path.exists(),
        "mcp.json with only tokensave should be removed on uninstall"
    );

    let agents_md = home.join(".kimi/AGENTS.md");
    if agents_md.exists() {
        let content = std::fs::read_to_string(&agents_md).unwrap();
        assert!(
            !content.contains("## Prefer tokensave MCP tools"),
            "AGENTS.md should not have tokensave rules after uninstall"
        );
    }
}

#[test]
fn test_kimi_is_detected_and_has_tokensave() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    assert!(!KimiIntegration.is_detected(home));
    assert!(!KimiIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    KimiIntegration.install(&ctx).unwrap();

    assert!(KimiIntegration.is_detected(home));
    assert!(KimiIntegration.has_tokensave(home));
}

#[test]
fn test_cursor_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CursorIntegration.install(&ctx).unwrap();

    let mcp_path = home.join(".cursor/mcp.json");
    assert!(mcp_path.exists(), "mcp.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_opencode_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // OpenCode uses ~/.config/opencode/opencode.json
    // Create the parent dir so install can discover it
    let ctx = make_install_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let config_path = home.join(".config/opencode/opencode.json");
    assert!(
        config_path.exists(),
        "opencode.json should exist after install"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert!(content["mcp"]["tokensave"].is_object());
}

#[test]
fn test_zed_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ZedIntegration.install(&ctx).unwrap();

    // On macOS: ~/Library/Application Support/Zed/settings.json
    // On linux: ~/.config/zed/settings.json
    #[cfg(target_os = "macos")]
    let settings_path = home.join("Library/Application Support/Zed/settings.json");
    #[cfg(not(target_os = "macos"))]
    let settings_path = home.join(".config/zed/settings.json");

    assert!(
        settings_path.exists(),
        "Zed settings.json should exist after install"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["context_servers"]["tokensave"].is_object());
}

#[test]
fn test_cline_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClineIntegration.install(&ctx).unwrap();

    // Cline uses VS Code extension global storage
    #[cfg(target_os = "macos")]
    let settings_path = home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(target_os = "linux")]
    let settings_path = home.join(
        ".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    );
    #[cfg(target_os = "windows")]
    let settings_path = home.join("AppData/Roaming/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let settings_path = home.join(
        ".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    );

    assert!(
        settings_path.exists(),
        "Cline settings should exist after install"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_roo_code_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    RooCodeIntegration.install(&ctx).unwrap();

    #[cfg(target_os = "macos")]
    let settings_path = home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(target_os = "linux")]
    let settings_path = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(target_os = "windows")]
    let settings_path = home.join("AppData/Roaming/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let settings_path = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");

    assert!(
        settings_path.exists(),
        "Roo Code settings should exist after install"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_copilot_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    // Check VS Code settings.json
    #[cfg(target_os = "macos")]
    let vscode_settings = home.join("Library/Application Support/Code/User/settings.json");
    #[cfg(target_os = "linux")]
    let vscode_settings = home.join(".config/Code/User/settings.json");
    #[cfg(target_os = "windows")]
    let vscode_settings = home.join("AppData/Roaming/Code/User/settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let vscode_settings = home.join(".config/Code/User/settings.json");

    assert!(
        vscode_settings.exists(),
        "VS Code settings.json should exist"
    );
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&vscode_settings).unwrap()).unwrap();
    assert!(content["mcp"]["servers"]["tokensave"].is_object());

    // Check CLI config
    let cli_config = home.join(".copilot/mcp-config.json");
    assert!(cli_config.exists(), "Copilot CLI config should exist");
    let cli_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cli_config).unwrap()).unwrap();
    assert!(cli_content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_vibe_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    VibeIntegration.install(&ctx).unwrap();

    let config_path = home.join(".vibe/config.toml");
    assert!(
        config_path.exists(),
        "config.toml should exist after install"
    );
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("name = \"tokensave\""),
        "config should contain tokensave MCP server"
    );
    assert!(
        content.contains("transport = \"stdio\""),
        "config should use stdio transport"
    );
    assert!(
        content.contains("args = [\"serve\"]"),
        "config should have serve arg"
    );

    // Check prompt rules
    let prompt_path = home.join(".vibe/prompts/cli.md");
    assert!(
        prompt_path.exists(),
        "Vibe prompt should exist after install"
    );
    let prompt = std::fs::read_to_string(&prompt_path).unwrap();
    assert!(prompt.contains("tokensave"));
}

// ---------------------------------------------------------------------------
// 4. Install followed by Uninstall
// ---------------------------------------------------------------------------

#[test]
fn test_claude_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Install
    ClaudeIntegration.install(&ctx).unwrap();
    assert!(home.join(".claude.json").exists());

    // Uninstall
    ClaudeIntegration.uninstall(&ctx).unwrap();

    // ~/.claude.json should be removed (was only tokensave)
    // It may be removed entirely or have mcpServers removed
    if home.join(".claude.json").exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        // Should not have tokensave anymore
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(
            !has_tokensave,
            "tokensave should be removed from .claude.json after uninstall"
        );
    }
}

#[test]
fn test_gemini_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    GeminiIntegration.install(&ctx).unwrap();
    let settings_path = home.join(".gemini/settings.json");
    assert!(settings_path.exists());

    GeminiIntegration.uninstall(&ctx).unwrap();

    // After uninstall, settings.json should be removed or not contain tokensave
    if settings_path.exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(
            !has_tokensave,
            "tokensave should be removed from settings.json"
        );
    }

    // GEMINI.md should be removed (was only tokensave rules)
    let gemini_md = home.join(".gemini/GEMINI.md");
    if gemini_md.exists() {
        let content = std::fs::read_to_string(&gemini_md).unwrap();
        assert!(
            !content.contains("## Prefer tokensave MCP tools"),
            "GEMINI.md should not contain tokensave rules after uninstall"
        );
    }
}

#[test]
fn test_codex_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    CodexIntegration.install(&ctx).unwrap();
    let config_path = home.join(".codex/config.toml");
    assert!(config_path.exists());

    CodexIntegration.uninstall(&ctx).unwrap();

    // After uninstall, the config (which only contained tokensave) becomes
    // empty and is removed; or, if other content existed, the tokensave
    // server is dropped but the rest is preserved.
    assert!(
        !config_path.exists(),
        "config.toml with only tokensave should be removed on uninstall"
    );

    let agents_md = home.join(".codex/AGENTS.md");
    if agents_md.exists() {
        let content = std::fs::read_to_string(&agents_md).unwrap();
        assert!(
            !content.contains("## Prefer tokensave MCP tools"),
            "AGENTS.md should not have tokensave rules after uninstall"
        );
    }
}

#[test]
fn test_codex_install_preserves_existing_config() {
    // Regression test for issue #63: installing tokensave used to wipe out the
    // entire ~/.codex/config.toml because load_toml_file silently returned an
    // empty table.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    let config_path = home.join(".codex/config.toml");
    let original = "\
model = \"o4-mini\"
approval_policy = \"on-failure\"

[mcp_servers.other]
command = \"other-bin\"
args = [\"--flag\"]
";
    std::fs::write(&config_path, original).unwrap();

    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();

    // A backup of the original must exist.
    let backup = home.join(".codex/config.toml.bak");
    assert!(backup.exists(), "install must back up the existing config");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);

    // The new config must keep the user's settings.
    let new_contents = std::fs::read_to_string(&config_path).unwrap();
    let parsed: toml::Table = toml::from_str(&new_contents).unwrap();
    assert_eq!(
        parsed.get("model").and_then(|v| v.as_str()),
        Some("o4-mini"),
        "top-level user keys must be preserved"
    );
    assert_eq!(
        parsed.get("approval_policy").and_then(|v| v.as_str()),
        Some("on-failure"),
    );
    let servers = parsed
        .get("mcp_servers")
        .and_then(|v| v.as_table())
        .expect("mcp_servers should still be a table");
    assert!(
        servers.contains_key("other"),
        "pre-existing mcp_servers entries must be preserved"
    );
    assert!(
        servers.contains_key("tokensave"),
        "tokensave should be registered alongside existing servers"
    );
}

#[test]
fn test_codex_install_refuses_unparseable_config() {
    // Issue #63 guard: if the existing config can't be parsed, refuse to
    // overwrite rather than silently replacing the user's content.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    let config_path = home.join(".codex/config.toml");
    let original = "this is not valid TOML {{{{";
    std::fs::write(&config_path, original).unwrap();

    let ctx = make_install_ctx(home);
    let result = CodexIntegration.install(&ctx);
    assert!(
        result.is_err(),
        "install must fail when existing config.toml is unparseable"
    );
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        original,
        "the broken config must be left untouched so the user can fix it"
    );
}

// ---------------------------------------------------------------------------
// Issue #63 regression: every agent must back up an existing config before
// overwriting it, and the user's pre-existing content must survive install.
// ---------------------------------------------------------------------------

/// Seed the agent's primary config with `original`, run install, then assert
/// that a `.bak` was created with the original bytes and that the new content
/// still contains the user's `marker` substring.
///
/// The path is taken from `agent.primary_config_path(home)` so a future change
/// to platform-conditional path logic (e.g. zed v4.3.15 Windows incident)
/// can't drift between tests and production.
fn assert_install_backs_up_and_preserves(
    agent: &dyn AgentIntegration,
    home: &Path,
    original: &str,
    marker: &str,
) {
    let config_path = agent
        .primary_config_path(home)
        .unwrap_or_else(|| panic!("{} must implement primary_config_path", agent.name()));
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, original).unwrap();

    let ctx = make_install_ctx(home);
    agent.install(&ctx).expect("install should succeed");

    let mut backup = config_path.as_os_str().to_owned();
    backup.push(".bak");
    let backup = std::path::PathBuf::from(backup);
    assert!(
        backup.exists(),
        "{}: install must back up the existing config to {}",
        agent.name(),
        backup.display()
    );
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        original,
        "{}: backup must contain the exact original bytes",
        agent.name()
    );

    let new = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        new.contains(marker),
        "{}: user's pre-existing content (marker {marker:?}) must be preserved, got:\n{new}",
        agent.name(),
    );
}

#[test]
fn test_claude_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "theme": "solarized",
  "mcpServers": {
    "other": { "command": "other-bin", "args": ["--flag"] }
  }
}
"#;
    assert_install_backs_up_and_preserves(&ClaudeIntegration, dir.path(), original, "solarized");
}

#[test]
fn test_gemini_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "theme": "dark",
  "mcpServers": { "other": { "command": "other-bin" } }
}
"#;
    assert_install_backs_up_and_preserves(&GeminiIntegration, dir.path(), original, "\"theme\"");
}

#[test]
fn test_cursor_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "mcpServers": { "other": { "command": "other-bin" } }
}
"#;
    assert_install_backs_up_and_preserves(&CursorIntegration, dir.path(), original, "other-bin");
}

#[test]
fn test_opencode_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "$schema": "https://opencode.ai/config.json",
  "mcp": { "other": { "type": "local", "command": ["other-bin"] } }
}
"#;
    assert_install_backs_up_and_preserves(&OpenCodeIntegration, dir.path(), original, "other-bin");
}

#[test]
fn test_zed_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "theme": "One Dark",
  "context_servers": { "other": { "command": { "path": "other-bin", "args": [] } } }
}
"#;
    assert_install_backs_up_and_preserves(&ZedIntegration, dir.path(), original, "One Dark");
}

#[test]
fn test_cline_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "mcpServers": { "other": { "command": "other-bin" } }
}
"#;
    assert_install_backs_up_and_preserves(&ClineIntegration, dir.path(), original, "other-bin");
}

#[test]
fn test_roo_code_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "mcpServers": { "other": { "command": "other-bin" } }
}
"#;
    assert_install_backs_up_and_preserves(&RooCodeIntegration, dir.path(), original, "other-bin");
}

#[test]
fn test_cursor_uninstall_backs_up_config_with_other_content() {
    // Regression for issue #63: uninstall paths must also back up the file
    // before rewriting, so a botched rewrite is recoverable.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    let path = home.join(".cursor/mcp.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = r#"{
  "mcpServers": {
    "tokensave": { "command": "/usr/local/bin/tokensave", "args": ["serve"] },
    "other": { "command": "other-bin" }
  }
}
"#;
    std::fs::write(&path, original).unwrap();

    CursorIntegration.uninstall(&ctx).unwrap();

    let backup = home.join(".cursor/mcp.json.bak");
    assert!(
        backup.exists(),
        "uninstall must back up the existing config before rewriting it"
    );
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        original,
        "backup must contain the exact pre-uninstall bytes"
    );
    let new = std::fs::read_to_string(&path).unwrap();
    assert!(
        new.contains("other-bin") && !new.contains("tokensave"),
        "uninstall must drop tokensave but keep other servers; got:\n{new}"
    );
}

#[test]
fn test_kilo_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  // user comment about their workflow
  "mcp": { "other": { "type": "local", "command": ["other-bin"], "enabled": true } }
}
"#;
    assert_install_backs_up_and_preserves(&KiloIntegration, dir.path(), original, "other-bin");
}

#[test]
fn test_antigravity_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "mcpServers": { "other": { "command": "other-bin" } }
}
"#;
    assert_install_backs_up_and_preserves(
        &AntigravityIntegration,
        dir.path(),
        original,
        "other-bin",
    );
}

/// Regression for #85: `tokensave install --agent antigravity` must populate
/// both the IDE config and the CLI plugin file so the `agy` CLI can see the
/// server. Before the fix only the IDE path was written, which left the CLI
/// invisible in `/mcp`.
#[test]
fn test_antigravity_install_writes_cli_plugin() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let bin = "/usr/local/bin/tokensave";
    let ctx = InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: bin.to_string(),
        tool_permissions: expected_tool_perms(),
    };

    AntigravityIntegration.install(&ctx).expect("install ok");

    let ide_path = home.join(".gemini/antigravity/mcp_config.json");
    let cli_path = home.join(".gemini/antigravity-cli/plugins/tokensave.json");
    assert!(
        ide_path.exists(),
        "IDE config must be written: {ide_path:?}"
    );
    assert!(
        cli_path.exists(),
        "CLI plugin must be written: {cli_path:?}"
    );

    for path in [&ide_path, &cli_path] {
        let body: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let server = body
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .expect("tokensave entry");
        assert_eq!(
            server.get("command").and_then(|v| v.as_str()),
            Some(bin),
            "{path:?} must point at the install bin"
        );
        assert!(
            server
                .get("args")
                .and_then(|v| v.as_array())
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("serve"))),
            "{path:?} must invoke `serve`"
        );
    }
}

/// Uninstall must remove the CLI plugin file outright (it belongs only to
/// tokensave) and remove the `tokensave` entry from the shared IDE config
/// without touching the user's other entries.
#[test]
fn test_antigravity_uninstall_removes_both_locations() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let bin = "/usr/local/bin/tokensave";
    let ctx = InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: bin.to_string(),
        tool_permissions: expected_tool_perms(),
    };

    AntigravityIntegration.install(&ctx).unwrap();
    AntigravityIntegration.uninstall(&ctx).unwrap();

    let cli_path = home.join(".gemini/antigravity-cli/plugins/tokensave.json");
    assert!(
        !cli_path.exists(),
        "CLI plugin file must be deleted, still exists at {cli_path:?}"
    );

    let ide_path = home.join(".gemini/antigravity/mcp_config.json");
    // IDE config either deleted (empty) or rewritten without our entry —
    // both are acceptable; what's not acceptable is the entry persisting.
    if ide_path.exists() {
        let body: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&ide_path).unwrap()).unwrap();
        assert!(
            body.get("mcpServers")
                .and_then(|v| v.get("tokensave"))
                .is_none(),
            "tokensave entry must be removed from {ide_path:?}"
        );
    }
}

#[test]
fn test_copilot_install_preserves_existing_config() {
    let dir = TempDir::new().unwrap();
    let original = r#"{
  "editor.fontSize": 14,
  "workbench.colorTheme": "Default Dark+"
}
"#;
    assert_install_backs_up_and_preserves(
        &CopilotIntegration,
        dir.path(),
        original,
        "Default Dark+",
    );
}

/// Meta-test: every agent that goes through `assert_install_backs_up_and_preserves`
/// above must also actually return a path from `primary_config_path`. Catches
/// the case where a new integration is added without wiring up the method,
/// which would otherwise only surface as a confusing panic in CI.
#[test]
fn test_every_tested_agent_advertises_primary_config_path() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let agents: Vec<(&dyn AgentIntegration, &str)> = vec![
        (&ClaudeIntegration, "claude"),
        (&GeminiIntegration, "gemini"),
        (&CursorIntegration, "cursor"),
        (&OpenCodeIntegration, "opencode"),
        (&ZedIntegration, "zed"),
        (&ClineIntegration, "cline"),
        (&RooCodeIntegration, "roo-code"),
        (&CopilotIntegration, "copilot"),
        (&KiloIntegration, "kilo"),
        (&AntigravityIntegration, "antigravity"),
        (&CodexIntegration, "codex"),
        (&KiroIntegration, "kiro"),
        (&KimiIntegration, "kimi"),
    ];
    for (agent, id) in agents {
        let path = agent
            .primary_config_path(home)
            .unwrap_or_else(|| panic!("{id} must implement primary_config_path"));
        assert!(
            path.starts_with(home),
            "{id}: primary_config_path must be under the home arg, got {}",
            path.display()
        );
    }
}

#[test]
fn test_cursor_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    CursorIntegration.install(&ctx).unwrap();
    let mcp_path = home.join(".cursor/mcp.json");
    assert!(mcp_path.exists());

    CursorIntegration.uninstall(&ctx).unwrap();

    // mcp.json should be removed (was only tokensave)
    if mcp_path.exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(!has_tokensave, "tokensave should be removed from mcp.json");
    }
}

#[test]
fn test_copilot_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    // CLI config should be cleaned up
    let cli_config = home.join(".copilot/mcp-config.json");
    if cli_config.exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cli_config).unwrap()).unwrap();
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(!has_tokensave);
    }
}

#[test]
fn test_vibe_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    VibeIntegration.install(&ctx).unwrap();
    VibeIntegration.uninstall(&ctx).unwrap();

    let config_path = home.join(".vibe/config.toml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(
            !content.contains("name = \"tokensave\""),
            "tokensave should be removed from config.toml"
        );
    }

    let prompt_path = home.join(".vibe/prompts/cli.md");
    if prompt_path.exists() {
        let content = std::fs::read_to_string(&prompt_path).unwrap();
        assert!(
            !content.contains("tokensave"),
            "tokensave rules should be removed from prompt"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Healthcheck with tempdir
// ---------------------------------------------------------------------------

/// Creates a fake tokensave binary in a temp dir and returns the path string.
/// This allows healthchecks to verify binary existence.
fn make_install_ctx_with_real_bin(home: &Path) -> InstallContext {
    let bin_dir = home.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let bin_path = bin_dir.join("tokensave");
    std::fs::write(&bin_path, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: bin_path.to_string_lossy().to_string(),
        tool_permissions: expected_tool_perms(),
    }
}

#[test]
fn test_healthcheck_claude_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx_with_real_bin(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Claude install should have no issues");
}

#[test]
fn test_healthcheck_gemini_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    GeminiIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Gemini install should have no issues");
}

#[test]
fn test_healthcheck_codex_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CodexIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(
        dc.issues, 0,
        "Codex healthcheck should pass after a clean install"
    );
}

#[test]
fn test_healthcheck_cursor_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CursorIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CursorIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Cursor install should have no issues");
}

#[test]
fn test_healthcheck_opencode_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean OpenCode install should have no issues");
}

#[test]
fn test_healthcheck_no_install_warns() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Healthcheck without installing should produce warnings (not crashes)
    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    // Should have issues (missing config files)
    assert!(
        dc.issues > 0 || dc.warnings > 0,
        "healthcheck on empty dir should report issues or warnings"
    );
}

#[test]
fn test_doctor_counters() {
    let mut dc = DoctorCounters::new();
    assert_eq!(dc.issues, 0);
    assert_eq!(dc.warnings, 0);

    dc.pass("this is fine");
    assert_eq!(dc.issues, 0);
    assert_eq!(dc.warnings, 0);

    dc.fail("something broke");
    assert_eq!(dc.issues, 1);
    assert_eq!(dc.warnings, 0);

    dc.warn("be careful");
    assert_eq!(dc.issues, 1);
    assert_eq!(dc.warnings, 1);

    dc.info("just info");
    assert_eq!(dc.issues, 1);
    assert_eq!(dc.warnings, 1);

    dc.fail("another failure");
    assert_eq!(dc.issues, 2);
    assert_eq!(dc.warnings, 1);
}

// ---------------------------------------------------------------------------
// 6. Helper function tests
// ---------------------------------------------------------------------------

#[test]
fn test_load_json_file_missing() {
    let val = load_json_file(Path::new("/nonexistent/file.json"));
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_valid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(&path, r#"{"key": "value"}"#).unwrap();
    let val = load_json_file(&path);
    assert_eq!(val["key"], "value");
}

#[test]
fn test_load_json_file_invalid_returns_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not valid json").unwrap();
    let val = load_json_file(&path);
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_strict_missing() {
    let result = load_json_file_strict(Path::new("/nonexistent/file.json"));
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_strict_empty_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.json");
    std::fs::write(&path, "").unwrap();
    let result = load_json_file_strict(&path);
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_strict_whitespace_only() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ws.json");
    std::fs::write(&path, "   \n  \t  ").unwrap();
    let result = load_json_file_strict(&path);
    assert!(result.is_ok());
}

#[test]
fn test_load_json_file_strict_invalid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not valid json").unwrap();
    assert!(load_json_file_strict(&path).is_err());
}

#[test]
fn test_load_json_file_strict_valid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("good.json");
    std::fs::write(&path, r#"{"hello": "world"}"#).unwrap();
    let val = load_json_file_strict(&path).unwrap();
    assert_eq!(val["hello"], "world");
}

#[test]
fn test_backup_config_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, r#"{"original": true}"#).unwrap();
    let backup = backup_config_file(&path).unwrap();
    assert!(backup.is_some());
    let backup_path = backup.unwrap();
    assert!(backup_path.exists());
    // Verify backup content matches original
    let backup_content = std::fs::read_to_string(&backup_path).unwrap();
    assert_eq!(backup_content, r#"{"original": true}"#);
}

#[test]
fn test_backup_config_file_missing() {
    let result = backup_config_file(Path::new("/nonexistent/file.json")).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_safe_write_json_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("output.json");
    let value = serde_json::json!({"hello": "world"});
    safe_write_json_file(&path, &value, None).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["hello"], "world");
}

#[test]
fn test_safe_write_json_file_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep/nested/dir/output.json");
    let value = serde_json::json!({"nested": true});
    safe_write_json_file(&path, &value, None).unwrap();
    assert!(path.exists());
}

#[test]
fn test_safe_write_json_file_overwrites_existing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("existing.json");
    std::fs::write(&path, r#"{"old": true}"#).unwrap();
    let value = serde_json::json!({"new": true});
    safe_write_json_file(&path, &value, None).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["new"], true);
    assert!(content.get("old").is_none());
}

#[test]
fn test_write_json_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("write_test.json");
    let value = serde_json::json!({"test": 42});
    write_json_file(&path, &value).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["test"], 42);
}

#[test]
fn test_load_toml_file_missing() {
    let val = load_toml_file(Path::new("/nonexistent/file.toml")).unwrap();
    assert!(val.is_table());
    assert!(val.as_table().unwrap().is_empty());
}

#[test]
fn test_load_toml_file_valid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.toml");
    std::fs::write(&path, "key = \"value\"\nnumber = 42\n").unwrap();
    let val = load_toml_file(&path).expect("valid TOML should parse as document");
    let table = val.as_table().expect("top-level should be a table");
    assert_eq!(table.get("key").and_then(|v| v.as_str()), Some("value"));
    assert_eq!(table.get("number").and_then(|v| v.as_integer()), Some(42));
}

#[test]
fn test_load_toml_file_invalid_returns_err() {
    // Bug #63: invalid TOML used to silently return an empty table, which let
    // install_mcp_server wipe out the user's config. Now it must surface an
    // error so the caller refuses to overwrite.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "{{{{not valid toml").unwrap();
    assert!(
        load_toml_file(&path).is_err(),
        "unparseable TOML must surface as error, not silently empty"
    );
}

#[test]
fn test_load_toml_file_empty_file_returns_empty_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.toml");
    std::fs::write(&path, "").unwrap();
    let val = load_toml_file(&path).expect("empty file should be treated as empty table");
    assert!(val.as_table().unwrap().is_empty());
}

#[test]
fn test_write_toml_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("output.toml");
    let mut table = toml::map::Map::new();
    table.insert("key".to_string(), toml::Value::String("value".to_string()));
    let val = toml::Value::Table(table);
    write_toml_file(&path, &val).unwrap();
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("key"));
    assert!(content.contains("value"));
}

#[test]
fn test_write_toml_file_backs_up_existing() {
    // Issue #63: overwriting an existing config must always leave a .bak copy
    // so the user can recover if anything goes wrong.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    let original = "preserved = \"keep me\"\n";
    std::fs::write(&path, original).unwrap();

    let mut table = toml::map::Map::new();
    table.insert(
        "new".to_string(),
        toml::Value::String("content".to_string()),
    );
    write_toml_file(&path, &toml::Value::Table(table)).unwrap();

    let backup = dir.path().join("config.toml.bak");
    assert!(
        backup.exists(),
        "write must create a .bak of the prior file"
    );
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        original,
        "the backup must contain the exact previous bytes"
    );
}

#[test]
fn test_write_toml_file_no_backup_when_no_prior_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.toml");
    let mut table = toml::map::Map::new();
    table.insert("k".to_string(), toml::Value::String("v".to_string()));
    write_toml_file(&path, &toml::Value::Table(table)).unwrap();

    let backup = dir.path().join("fresh.toml.bak");
    assert!(
        !backup.exists(),
        "no backup should be created on first write"
    );
}

// ---------------------------------------------------------------------------
// JSONC helpers
// ---------------------------------------------------------------------------

#[test]
fn test_load_jsonc_file_missing() {
    let val = load_jsonc_file(Path::new("/nonexistent/file.jsonc"));
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_jsonc_file_with_comments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.jsonc");
    std::fs::write(
        &path,
        r#"{
        // This is a comment
        "key": "value", // trailing comment
        /* block comment */
        "number": 42,
    }"#,
    )
    .unwrap();
    let val = load_jsonc_file(&path);
    assert_eq!(val["key"], "value");
    assert_eq!(val["number"], 42);
}

#[test]
fn test_load_jsonc_file_strict_missing() {
    let result = load_jsonc_file_strict(Path::new("/nonexistent/file.jsonc"));
    assert!(result.is_ok());
}

#[test]
fn test_load_jsonc_file_strict_with_comments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.jsonc");
    std::fs::write(
        &path,
        r#"{
        // comment
        "key": "value"
    }"#,
    )
    .unwrap();
    let val = load_jsonc_file_strict(&path).unwrap();
    assert_eq!(val["key"], "value");
}

#[test]
fn test_parse_jsonc() {
    let input = r#"{
        // line comment
        "a": 1,
        /* block */ "b": 2,
    }"#;
    let val = parse_jsonc(input);
    assert_eq!(val["a"], 1);
    assert_eq!(val["b"], 2);
}

// ---------------------------------------------------------------------------
// 7. is_detected / has_tokensave tests
// ---------------------------------------------------------------------------

#[test]
fn test_is_detected_claude() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!ClaudeIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    assert!(ClaudeIntegration.is_detected(home));
}

#[test]
fn test_is_detected_codex() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CodexIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    assert!(CodexIntegration.is_detected(home));
}

#[test]
fn test_is_detected_gemini() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!GeminiIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".gemini")).unwrap();
    assert!(GeminiIntegration.is_detected(home));
}

#[test]
fn test_is_detected_cursor() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CursorIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".cursor")).unwrap();
    assert!(CursorIntegration.is_detected(home));
}

#[test]
fn test_is_detected_opencode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!OpenCodeIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".config/opencode")).unwrap();
    assert!(OpenCodeIntegration.is_detected(home));
}

#[test]
fn test_is_detected_zed() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!ZedIntegration.is_detected(home));
    #[cfg(target_os = "macos")]
    std::fs::create_dir_all(home.join("Library/Application Support/Zed")).unwrap();
    #[cfg(not(target_os = "macos"))]
    std::fs::create_dir_all(home.join(".config/zed")).unwrap();
    assert!(ZedIntegration.is_detected(home));
}

#[test]
fn test_is_detected_copilot() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // Copilot is detected when either VS Code User dir or .copilot dir exists
    assert!(!CopilotIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".copilot")).unwrap();
    assert!(CopilotIntegration.is_detected(home));
}

#[test]
fn test_has_tokensave_claude() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // No config => false
    assert!(!ClaudeIntegration.has_tokensave(home));

    // After install => true
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();
    assert!(ClaudeIntegration.has_tokensave(home));

    // After uninstall => false
    ClaudeIntegration.uninstall(&ctx).unwrap();
    assert!(!ClaudeIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_gemini() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!GeminiIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();
    assert!(GeminiIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_codex() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CodexIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();
    assert!(home.join(".codex/config.toml").exists());
    assert!(
        CodexIntegration.has_tokensave(home),
        "has_tokensave should detect tokensave after a clean install"
    );
}

#[test]
fn test_has_tokensave_cursor() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CursorIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    CursorIntegration.install(&ctx).unwrap();
    assert!(CursorIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_opencode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!OpenCodeIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    assert!(OpenCodeIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_copilot() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CopilotIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    assert!(CopilotIntegration.has_tokensave(home));
}

// ---------------------------------------------------------------------------
// 8. Idempotency tests
// ---------------------------------------------------------------------------

#[test]
fn test_claude_install_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Install twice should not fail
    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.install(&ctx).unwrap();

    // Config should still be valid
    let claude_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
    assert!(claude_json["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_gemini_install_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    GeminiIntegration.install(&ctx).unwrap();
    GeminiIntegration.install(&ctx).unwrap();

    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".gemini/settings.json")).unwrap())
            .unwrap();
    assert!(settings["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Uninstalling when nothing is installed should not panic or error
    ClaudeIntegration.uninstall(&ctx).unwrap();
    GeminiIntegration.uninstall(&ctx).unwrap();
    CodexIntegration.uninstall(&ctx).unwrap();
    CursorIntegration.uninstall(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();
    ZedIntegration.uninstall(&ctx).unwrap();
    ClineIntegration.uninstall(&ctx).unwrap();
    RooCodeIntegration.uninstall(&ctx).unwrap();
    KiroIntegration.uninstall(&ctx).unwrap();
    VibeIntegration.uninstall(&ctx).unwrap();
}

// ---------------------------------------------------------------------------
// 9. Install preserves existing config
// ---------------------------------------------------------------------------

#[test]
fn test_claude_install_preserves_existing_claude_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate .claude.json with other data
    let claude_json_path = home.join(".claude.json");
    std::fs::write(
        &claude_json_path,
        r#"{"mcpServers": {"other-server": {"command": "foo"}}, "customKey": 42}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json_path).unwrap()).unwrap();
    // tokensave added
    assert!(content["mcpServers"]["tokensave"].is_object());
    // existing server preserved
    assert!(content["mcpServers"]["other-server"].is_object());
    // custom key preserved
    assert_eq!(content["customKey"], 42);
}

#[test]
fn test_gemini_install_preserves_existing_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let settings_path = home.join(".gemini/settings.json");
    std::fs::create_dir_all(home.join(".gemini")).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcpServers": {"other": {"command": "bar"}}, "theme": "dark"}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
    assert!(content["mcpServers"]["other"].is_object());
    assert_eq!(content["theme"], "dark");
}

// ---------------------------------------------------------------------------
// 10. Constants sanity
// ---------------------------------------------------------------------------

#[test]
fn test_tool_names_not_empty() {
    let names = tool_names();
    assert!(!names.is_empty());
    for name in &names {
        assert!(
            name.starts_with("tokensave_"),
            "tool name should start with tokensave_: {name}"
        );
    }
}

#[test]
fn test_read_only_tool_names_excludes_mutating_tools() {
    let read_only = read_only_tool_names();
    let read_only_set: std::collections::HashSet<&str> =
        read_only.iter().map(String::as_str).collect();
    let known_tools: std::collections::HashSet<String> = tool_names().into_iter().collect();
    assert!(!read_only.is_empty());

    for name in &read_only {
        assert!(
            known_tools.contains(name),
            "read-only tool should be a known MCP tool: {name}"
        );
    }

    for mutating in [
        "tokensave_str_replace",
        "tokensave_multi_str_replace",
        "tokensave_insert_at",
        "tokensave_ast_grep_rewrite",
        "tokensave_session_start",
        "tokensave_session_end",
        "tokensave_record_decision",
        "tokensave_record_code_area",
    ] {
        assert!(
            !read_only_set.contains(mutating),
            "mutating tool should not be read-only: {mutating}"
        );
    }
}

#[test]
fn test_expected_tool_perms_not_empty() {
    let perms = expected_tool_perms();
    assert!(!perms.is_empty());
    for perm in &perms {
        assert!(
            perm.starts_with("mcp__tokensave__"),
            "tool perm should start with mcp__tokensave__: {perm}"
        );
    }
}

#[test]
fn test_tool_perms_match_tool_names() {
    let names = tool_names();
    let perms = expected_tool_perms();
    assert_eq!(
        names.len(),
        perms.len(),
        "tool_names and expected_tool_perms should have same length"
    );
    for name in &names {
        let expected_perm = format!("mcp__tokensave__{name}");
        assert!(
            perms.contains(&expected_perm),
            "missing permission for tool {name}: expected {expected_perm}"
        );
    }
}

// ---------------------------------------------------------------------------
// 11. restore_config_backup
// ---------------------------------------------------------------------------

#[test]
fn test_restore_config_backup_restores_content() {
    let dir = TempDir::new().unwrap();
    let original_path = dir.path().join("config.json");
    let backup_path = dir.path().join("config.json.bak");

    // Create original and backup
    std::fs::write(&original_path, r#"{"version": 1}"#).unwrap();
    std::fs::write(&backup_path, r#"{"version": 1}"#).unwrap();

    // Corrupt the original
    std::fs::write(&original_path, "CORRUPTED").unwrap();

    // Restore from backup
    restore_config_backup(&original_path, &backup_path);

    let restored = std::fs::read_to_string(&original_path).unwrap();
    assert_eq!(
        restored, r#"{"version": 1}"#,
        "restored content should match the backup"
    );
}

#[test]
fn test_restore_config_backup_to_missing_original() {
    let dir = TempDir::new().unwrap();
    let original_path = dir.path().join("config.json");
    let backup_path = dir.path().join("config.json.bak");

    // Only create backup, not original
    std::fs::write(&backup_path, r#"{"saved": true}"#).unwrap();

    restore_config_backup(&original_path, &backup_path);

    assert!(
        original_path.exists(),
        "original should be created from backup"
    );
    let content = std::fs::read_to_string(&original_path).unwrap();
    assert_eq!(content, r#"{"saved": true}"#);
}

#[test]
fn test_restore_config_backup_missing_backup_does_not_panic() {
    let dir = TempDir::new().unwrap();
    let original_path = dir.path().join("config.json");
    let backup_path = dir.path().join("config.json.bak");

    std::fs::write(&original_path, "original").unwrap();

    // Restore with a nonexistent backup — should not panic
    restore_config_backup(&original_path, &backup_path);

    // Original should remain untouched since backup failed
    let content = std::fs::read_to_string(&original_path).unwrap();
    assert_eq!(content, "original");
}

// ---------------------------------------------------------------------------
// 12. which_tokensave
// ---------------------------------------------------------------------------

#[test]
fn test_which_tokensave_returns_some_or_none() {
    // which_tokensave checks current_exe and PATH — we just verify it
    // doesn't panic and returns a sensible result.
    let result = which_tokensave();
    // In a test environment, the current exe is the test runner, not tokensave,
    // so it may return None (unless tokensave is on PATH). Either way, no panic.
    if let Some(ref path) = result {
        assert!(!path.is_empty(), "path should not be empty if Some");
    }
    // Test passes regardless of Some or None — just ensures no panic.
}

// ---------------------------------------------------------------------------
// 13. home_dir
// ---------------------------------------------------------------------------

#[test]
fn test_home_dir_returns_some() {
    let result = home_dir();
    assert!(
        result.is_some(),
        "home_dir should return Some on most systems"
    );
    let home = result.unwrap();
    assert!(home.is_absolute(), "home dir should be an absolute path");
}

// ---------------------------------------------------------------------------
// 14. migrate_installed_agents
// ---------------------------------------------------------------------------

#[test]
fn test_migrate_installed_agents_skips_when_already_populated() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let mut config = tokensave::user_config::UserConfig {
        installed_agents: vec!["claude".to_string()],
        ..Default::default()
    };

    // Should return immediately since installed_agents is non-empty
    migrate_installed_agents(home, &mut config);

    // The existing list should be unchanged
    assert_eq!(config.installed_agents, vec!["claude".to_string()]);
}

#[test]
fn test_migrate_installed_agents_detects_installed_agents() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Install copilot so it can be detected
    let ctx = make_install_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let mut config = tokensave::user_config::UserConfig::default();
    assert!(config.installed_agents.is_empty());

    // migrate will scan and detect copilot is installed
    // Note: save() will try to write to ~/.tokensave/config.toml which may fail
    // in CI, but the function still populates installed_agents in memory.
    migrate_installed_agents(home, &mut config);

    assert!(
        config.installed_agents.contains(&"copilot".to_string()),
        "copilot should be detected, got: {:?}",
        config.installed_agents
    );
}

#[test]
fn test_migrate_installed_agents_empty_home_no_change() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let mut config = tokensave::user_config::UserConfig::default();

    migrate_installed_agents(home, &mut config);

    // No agents installed in empty home, list should remain empty
    assert!(
        config.installed_agents.is_empty(),
        "installed_agents should remain empty when no agents detected"
    );
}

// ---------------------------------------------------------------------------
// 15. pick_integrations_interactive (no-agent-detected error path)
// ---------------------------------------------------------------------------

#[test]
fn test_pick_integrations_interactive_no_agents_detected() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Empty home — no agents detected
    let result = pick_integrations_interactive(home, &[]);
    assert!(
        result.is_err(),
        "pick_integrations_interactive should error when no agents detected"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("No supported agents detected"),
        "error should mention no agents detected, got: {err_msg}"
    );
}

#[test]
fn test_pick_integrations_interactive_single_uninstalled_agent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create only the .copilot dir so exactly one agent is detected
    std::fs::create_dir_all(home.join(".copilot")).unwrap();

    // Single detected agent that is NOT installed => fast path returns it directly
    let result = pick_integrations_interactive(home, &[]);
    assert!(
        result.is_ok(),
        "should succeed with single uninstalled agent"
    );
    let (to_install, to_uninstall) = result.unwrap();
    assert_eq!(to_install, vec!["copilot".to_string()]);
    assert!(to_uninstall.is_empty());
}

// ---------------------------------------------------------------------------
// 16. vscode_data_dir / copilot_cli_dir
// ---------------------------------------------------------------------------

#[test]
fn test_vscode_data_dir_is_under_home() {
    let home = Path::new("/fake/home");
    let dir = tokensave::agents::vscode_data_dir(home);
    assert!(
        dir.starts_with("/fake/home"),
        "vscode_data_dir should be under home: {}",
        dir.display()
    );
}

#[test]
fn test_copilot_cli_dir_is_under_home() {
    let home = Path::new("/fake/home");
    let dir = tokensave::agents::copilot_cli_dir(home);
    assert_eq!(
        dir,
        Path::new("/fake/home/.copilot"),
        "copilot_cli_dir should be home/.copilot"
    );
}

// ---------------------------------------------------------------------------
// 17. parse_jsonc edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_parse_jsonc_empty_string() {
    let val = parse_jsonc("");
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_parse_jsonc_only_comments() {
    let input = "// just a comment\n/* block */\n";
    let val = parse_jsonc(input);
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_parse_jsonc_nested_comments() {
    let input = r#"{
        "a": "hello // not a comment",
        /* this is a real comment */
        "b": true
    }"#;
    let val = parse_jsonc(input);
    assert_eq!(val["a"].as_str().unwrap(), "hello // not a comment");
    assert_eq!(val["b"], true);
}

#[test]
fn test_parse_jsonc_trailing_comma_in_object() {
    let input = r#"{"a": 1, "b": 2,}"#;
    let val = parse_jsonc(input);
    assert_eq!(val["a"], 1);
    assert_eq!(val["b"], 2);
}

#[test]
fn test_parse_jsonc_trailing_comma_in_array() {
    let input = r#"{"arr": [1, 2, 3,]}"#;
    let val = parse_jsonc(input);
    let arr = val["arr"].as_array().unwrap();
    assert_eq!(arr.len(), 3);
}

// ---------------------------------------------------------------------------
// 18. backup + safe_write round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_backup_and_safe_write_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("roundtrip.json");

    // Create initial file
    let initial = serde_json::json!({"name": "tokensave", "version": 1});
    safe_write_json_file(&path, &initial, None).unwrap();

    // Create backup
    let backup = backup_config_file(&path).unwrap();
    assert!(backup.is_some());
    let backup_path = backup.unwrap();

    // Overwrite with new content
    let updated = serde_json::json!({"name": "tokensave", "version": 2});
    safe_write_json_file(&path, &updated, Some(&backup_path)).unwrap();

    // Verify new content
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["version"], 2);

    // Verify backup still has old content
    let backup_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&backup_path).unwrap()).unwrap();
    assert_eq!(backup_content["version"], 1);

    // Restore from backup
    restore_config_backup(&path, &backup_path);
    let restored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(restored["version"], 1);
}
