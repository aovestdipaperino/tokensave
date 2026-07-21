use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::{
    AgentIntegration, DoctorCounters, HealthcheckContext, OpenCodeIntegration,
};

mod common;
use common::{make_install_ctx as make_ctx, read_json};

fn opencode_config_path(home: &Path) -> std::path::PathBuf {
    home.join(".config/opencode/opencode.json")
}

fn opencode_prompt_path(home: &Path) -> std::path::PathBuf {
    // Mirrors the logic: if .config/opencode exists, use it
    let modern = home.join(".config/opencode/AGENTS.md");
    if modern.exists() || home.join(".config/opencode").exists() {
        return modern;
    }
    home.join("AGENTS.md")
}

/// Path to the managed tokensave rules file, colocated with opencode.json.
fn opencode_rules_path(home: &Path) -> std::path::PathBuf {
    opencode_config_path(home)
        .parent()
        .unwrap()
        .join("tokensave.md")
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_opencode_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let config_path = opencode_config_path(home);
    assert!(config_path.exists(), "opencode.json should be created");

    let config = read_json(&config_path);
    let ts = &config["mcp"]["tokensave"];
    assert!(ts.is_object(), "mcp.tokensave should be an object");
    assert_eq!(
        ts["type"].as_str().unwrap(),
        "local",
        "type should be local"
    );

    let command: Vec<&str> = ts["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        command,
        vec!["/usr/local/bin/tokensave", "serve"],
        "command should be [bin, \"serve\"]"
    );

    let instructions: Vec<&str> = config["instructions"]
        .as_array()
        .expect("instructions array should be created")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    // Global installs must use an absolute path: OpenCode resolves a bare
    // relative entry by globbing upward from the *project* currently being
    // worked on, not from opencode.json's own directory, so a plain
    // "tokensave.md" here would never actually resolve to the global file.
    let expected = opencode_rules_path(home);
    assert_eq!(
        instructions,
        vec![expected.to_string_lossy()],
        "instructions should reference the managed rules file by absolute path"
    );
}

#[test]
fn test_install_creates_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    // Issue #256: rules live in a tokensave-owned managed file referenced from
    // opencode.json's "instructions" array, not appended to AGENTS.md.
    assert!(
        !opencode_prompt_path(home).exists(),
        "install should not create/touch AGENTS.md"
    );

    let rules_path = opencode_rules_path(home);
    assert!(rules_path.exists(), "managed rules file should be created");

    let content = std::fs::read_to_string(&rules_path).unwrap();
    assert!(
        content.contains("## Prefer tokensave MCP tools"),
        "managed rules file should contain the tokensave rules marker"
    );
    assert!(
        content.contains("tokensave_context"),
        "managed rules file should mention tokensave tools"
    );
}

#[test]
fn test_install_preserves_existing_opencode_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate opencode.json with existing content
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        r#"{"theme": "dark", "mcp": {"other-tool": {"type": "local", "command": ["other"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let config = read_json(&config_path);
    assert_eq!(
        config["theme"].as_str().unwrap(),
        "dark",
        "existing settings should be preserved"
    );
    assert!(
        config["mcp"]["other-tool"].is_object(),
        "existing MCP server should be preserved"
    );
    assert!(
        config["mcp"]["tokensave"].is_object(),
        "tokensave should be added"
    );
}

#[test]
fn test_install_idempotent_opencode_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.install(&ctx).unwrap();

    let config = read_json(&opencode_config_path(home));
    let mcp = config["mcp"].as_object().unwrap();
    let ts_count = mcp.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once");
}

#[test]
fn test_install_idempotent_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.install(&ctx).unwrap();

    let content = std::fs::read_to_string(opencode_rules_path(home)).unwrap();
    let marker = "## Prefer tokensave MCP tools";
    let count = content.matches(marker).count();
    assert_eq!(
        count, 1,
        "marker should appear exactly once after double install, found {count}"
    );

    let config = read_json(&opencode_config_path(home));
    let instructions = config["instructions"].as_array().unwrap();
    assert_eq!(
        instructions.len(),
        1,
        "instructions array should not accumulate duplicate entries"
    );
}

#[test]
fn test_install_does_not_touch_existing_agents_md() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create AGENTS.md with pre-existing content
    let config_dir = home.join(".config/opencode");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("AGENTS.md"),
        "## My Custom Rules\n\nAlways use TypeScript.\n",
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let content = std::fs::read_to_string(config_dir.join("AGENTS.md")).unwrap();
    assert_eq!(
        content, "## My Custom Rules\n\nAlways use TypeScript.\n",
        "AGENTS.md should be left untouched by install"
    );

    let rules_content = std::fs::read_to_string(config_dir.join("tokensave.md")).unwrap();
    assert!(
        rules_content.contains("Prefer tokensave MCP tools"),
        "managed rules file should contain tokensave rules"
    );
}

#[test]
fn test_install_migrates_legacy_agents_md_block() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate AGENTS.md with the pre-#256 inline block.
    let config_dir = home.join(".config/opencode");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("AGENTS.md"),
        "## My Custom Rules\n\nAlways use TypeScript.\n\n\
        ## Prefer tokensave MCP tools\n\n\
        Before reading source files ... some legacy text.\n",
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let content = std::fs::read_to_string(config_dir.join("AGENTS.md")).unwrap();
    assert!(
        !content.contains("Prefer tokensave MCP tools"),
        "install should strip the legacy inline block from AGENTS.md"
    );
    assert!(
        content.contains("My Custom Rules"),
        "install should preserve unrelated AGENTS.md content"
    );

    let rules_content = std::fs::read_to_string(config_dir.join("tokensave.md")).unwrap();
    assert!(
        rules_content.contains("Prefer tokensave MCP tools"),
        "managed rules file should contain the rules (no duplication, no loss)"
    );
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_mcp_from_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();

    let config_path = opencode_config_path(home);
    // When tokensave was the only content, file should be removed entirely
    if config_path.exists() {
        let config = read_json(&config_path);
        let has_tokensave = config.get("mcp").and_then(|v| v.get("tokensave")).is_some();
        assert!(!has_tokensave, "mcp.tokensave should be removed");
    }
}

#[test]
fn test_uninstall_removes_empty_config_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();

    let config_path = opencode_config_path(home);
    // Since tokensave was the only entry, the file should be deleted
    assert!(
        !config_path.exists(),
        "opencode.json should be deleted when empty"
    );
}

#[test]
fn test_uninstall_preserves_other_mcp_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate with another server
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        r#"{"mcp": {"other-tool": {"type": "local", "command": ["other"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();

    assert!(
        config_path.exists(),
        "config should still exist with other servers"
    );
    let config = read_json(&config_path);
    assert!(
        config["mcp"]["other-tool"].is_object(),
        "other server should be preserved"
    );
    let has_tokensave = config.get("mcp").and_then(|v| v.get("tokensave")).is_some();
    assert!(!has_tokensave, "tokensave should be removed");
}

#[test]
fn test_uninstall_removes_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    OpenCodeIntegration.install(&ctx).unwrap();
    let rules_path = opencode_rules_path(home);
    assert!(rules_path.exists());

    OpenCodeIntegration.uninstall(&ctx).unwrap();

    assert!(
        !rules_path.exists(),
        "managed rules file should be removed after uninstall"
    );
}

#[test]
fn test_uninstall_removes_instructions_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate with another server + another instructions entry.
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        r#"{"mcp": {"other-tool": {"type": "local", "command": ["other"]}}, "instructions": ["other.md"]}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();

    let config = read_json(&config_path);
    let instructions: Vec<&str> = config["instructions"]
        .as_array()
        .expect("instructions array should survive since other.md is still there")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        instructions,
        vec!["other.md"],
        "tokensave.md entry should be removed, other.md preserved"
    );
}

#[test]
fn test_uninstall_preserves_other_agents_md_content() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create AGENTS.md with pre-existing content
    let config_dir = home.join(".config/opencode");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("AGENTS.md"),
        "## My Custom Rules\n\nAlways use TypeScript.\n",
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();

    let prompt_path = config_dir.join("AGENTS.md");
    assert!(prompt_path.exists(), "AGENTS.md should still exist");
    let content = std::fs::read_to_string(&prompt_path).unwrap();
    assert_eq!(
        content, "## My Custom Rules\n\nAlways use TypeScript.\n",
        "AGENTS.md should be untouched throughout install+uninstall"
    );
    assert!(
        !config_dir.join("tokensave.md").exists(),
        "managed rules file should be removed after uninstall"
    );
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    // Should not panic or error
    OpenCodeIntegration.uninstall(&ctx).unwrap();
}

#[test]
fn test_uninstall_config_with_no_tokensave_is_noop() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create opencode.json without tokensave
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        r#"{"mcp": {"something-else": {"type": "local", "command": ["x"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    OpenCodeIntegration.uninstall(&ctx).unwrap();

    // File should remain with existing content
    let config = read_json(&config_path);
    assert!(config["mcp"]["something-else"].is_object());
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_clean_install_no_issues() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
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
fn test_healthcheck_missing_config_produces_warnings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0 || dc.issues > 0,
        "healthcheck on empty dir should report warnings or issues"
    );
}

#[test]
fn test_healthcheck_detects_missing_mcp_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create opencode.json without tokensave
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, r#"{"theme": "dark"}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert!(dc.issues > 0, "healthcheck should detect missing MCP entry");
}

#[test]
fn test_healthcheck_detects_missing_serve_arg() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create opencode.json with tokensave but no "serve" in command
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        r#"{"mcp": {"tokensave": {"type": "local", "command": ["/usr/local/bin/tokensave"]}}, "instructions": ["tokensave.md"]}"#,
    )
    .unwrap();

    // Also create the managed rules file so the prompt check passes
    std::fs::write(
        opencode_rules_path(home),
        "## Prefer tokensave MCP tools\ntokensave rules here\n",
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect missing 'serve' in command array"
    );
}

#[test]
fn test_healthcheck_detects_missing_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    std::fs::remove_file(opencode_rules_path(home)).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should flag a missing managed rules file"
    );
}

#[test]
fn test_healthcheck_detects_missing_tokensave_rules_in_managed_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    // Overwrite the managed rules file without any mention of tokensave
    std::fs::write(
        opencode_rules_path(home),
        "## Some other content\n\nGeneric rules only.\n",
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect missing tokensave rules in the managed file"
    );
}

#[test]
fn test_healthcheck_detects_missing_instructions_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    // Strip the instructions entry from opencode.json while leaving the
    // managed rules file itself in place.
    let config_path = opencode_config_path(home);
    let mut config = read_json(&config_path);
    config.as_object_mut().unwrap().remove("instructions");
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect a missing instructions entry"
    );
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_is_detected_empty_home() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !OpenCodeIntegration.is_detected(home),
        "should not be detected on empty home"
    );
}

#[test]
fn test_is_detected_with_opencode_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".config/opencode")).unwrap();
    assert!(
        OpenCodeIntegration.is_detected(home),
        "should be detected when .config/opencode exists"
    );
}

#[test]
fn test_has_tokensave_before_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !OpenCodeIntegration.has_tokensave(home),
        "has_tokensave should be false before install"
    );
}

#[test]
fn test_has_tokensave_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    assert!(
        OpenCodeIntegration.has_tokensave(home),
        "has_tokensave should be true after install"
    );
}

#[test]
fn test_has_tokensave_after_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();
    assert!(
        !OpenCodeIntegration.has_tokensave(home),
        "has_tokensave should be false after uninstall"
    );
}

#[test]
fn test_has_tokensave_with_config_but_no_mcp() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create opencode.json without mcp section
    let config_path = opencode_config_path(home);
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, r#"{"theme": "dark"}"#).unwrap();

    assert!(
        !OpenCodeIntegration.has_tokensave(home),
        "has_tokensave should be false when mcp section is missing"
    );
}

// ===========================================================================
// Name / ID
// ===========================================================================

#[test]
fn test_name_and_id() {
    assert_eq!(OpenCodeIntegration.name(), "OpenCode");
    assert_eq!(OpenCodeIntegration.id(), "opencode");
}
