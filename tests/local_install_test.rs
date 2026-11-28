use tempfile::TempDir;
use tokensave::agents::get_integration;
use tokensave::agents::{DoctorCounters, HealthcheckContext, InstallContext, InstallScope};

use crate::common::read_json;

#[test]
fn codex_local_mcp_install_and_uninstall_preserve_other_settings() {
    let home = TempDir::new().expect("create test home");
    let project = TempDir::new().expect("create test project");
    let project_config = project.path().join(".codex").join("config.toml");
    std::fs::create_dir(project.path().join(".codex")).expect("create project config directory");
    let original = r#"
model = "custom"

[mcp_servers.other]
command = "other"

[mcp_servers.tokensave]
startup_timeout_sec = 90
enabled_tools = ["tokensave_search"]
"#;
    let read_config =
        || std::fs::read_to_string(&project_config).expect("read project configuration");
    std::fs::write(&project_config, original).expect("write project configuration");
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "path with spaces/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: project.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    let ag = get_integration("codex").unwrap();
    assert!(ag.supports_local());
    ag.install(&ctx).expect("install project MCP registration");
    let installed = read_config();
    ag.install(&ctx)
        .expect("reinstall project MCP registration");
    assert_eq!(read_config(), installed);
    let config: toml::Table = toml::from_str(&installed).expect("parse installed TOML");
    let server = &config["mcp_servers"]["tokensave"];
    assert_eq!(server["command"].as_str(), Some(ctx.tokensave_bin.as_str()));
    assert_eq!(server["startup_timeout_sec"].as_integer(), Some(90));
    assert_eq!(
        server["enabled_tools"]
            .as_array()
            .expect("preserve tool filter")[0]
            .as_str(),
        Some("tokensave_search")
    );

    let custom_home = home.path().join("custom-codex");
    let user_config = custom_home.join("config.toml");
    std::fs::create_dir(&custom_home).expect("create custom Codex home");
    std::fs::write(&user_config, original).expect("write user-scoped MCP registration");
    let diagnostic = std::process::Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(project.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("CODEX_HOME", home.path().join("unused-codex-home"))
        .args(["doctor", "--agent", "codex"])
        .output()
        .expect("diagnose project-only Codex registration");
    assert!(
        String::from_utf8_lossy(&diagnostic.stderr).contains("Codex project MCP server registered"),
        "{diagnostic:?}"
    );
    let uninstall = |dry_run| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_tokensave"));
        command
            .current_dir(project.path())
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .env("CODEX_HOME", &custom_home)
            .args(["uninstall", "--local", "--agent", "codex", "--verbose"]);
        if dry_run {
            command.arg("--dry-run");
        }
        let output = command.output().expect("run local uninstall");
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("config.toml"));
        assert_eq!(
            std::fs::read_to_string(&user_config).expect("read user-scoped config"),
            original
        );
    };
    uninstall(true);
    assert_eq!(read_config(), installed);
    uninstall(false);
    let config: toml::Table = toml::from_str(&read_config()).expect("parse remaining TOML");
    assert!(config["mcp_servers"].get("tokensave").is_none());

    ag.uninstall(&ctx).expect("repeat project uninstall");
    assert_eq!(config["model"].as_str(), Some("custom"));
    assert_eq!(
        config["mcp_servers"]["other"]["command"].as_str(),
        Some("other")
    );
    assert!(!project.path().join(".codex").join("hooks.json").exists());
    assert!(!home.path().join(".codex").exists());

    let malformed = "[mcp_servers";
    std::fs::write(&project_config, malformed).expect("write malformed TOML");
    assert!(ag.install(&ctx).is_err());
    assert_eq!(read_config(), malformed);
}

#[test]
fn claude_supports_local() {
    assert!(get_integration("claude").unwrap().supports_local());
}

#[test]
fn claude_local_writes_project_files_only() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();

    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec!["mcp__tokensave__search".to_string()],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    get_integration("claude").unwrap().install(&ctx).unwrap();

    // Project files exist with tokensave registered.
    let mcp = read_json(&proj.path().join(".mcp.json"));
    assert!(mcp["mcpServers"]["tokensave"].is_object());
    let settings = read_json(&proj.path().join(".claude").join("settings.json"));
    assert!(settings["hooks"]["PreToolUse"].is_array());
    assert!(
        !proj.path().join("CLAUDE.md").exists(),
        "local install should not create/touch project CLAUDE.md"
    );
    assert!(proj
        .path()
        .join(".claude")
        .join("rules")
        .join("tokensave.md")
        .exists());

    // Global config under home was NOT touched.
    assert!(
        !home.path().join(".claude.json").exists(),
        "must not write global ~/.claude.json"
    );
    assert!(
        !home.path().join(".claude").join("settings.json").exists(),
        "must not write global settings"
    );
}

#[test]
fn doctor_preserves_valid_local_mcp_json() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let mcp_path = proj.path().join(".mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"tokensave":{"command":"/usr/bin/tokensave","args":["serve"]}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let ctx = HealthcheckContext {
        home: home.path().to_path_buf(),
        project_path: proj.path().to_path_buf(),
    };
    get_integration("claude")
        .unwrap()
        .healthcheck(&mut dc, &ctx);

    assert!(
        mcp_path.exists(),
        "doctor must NOT delete a valid local .mcp.json"
    );
}

#[test]
fn cursor_local_writes_project_mcp_only() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    get_integration("cursor").unwrap().install(&ctx).unwrap();
    let mcp = read_json(&proj.path().join(".cursor").join("mcp.json"));
    assert!(mcp["mcpServers"]["tokensave"].is_object());
    assert!(
        !home.path().join(".cursor").join("mcp.json").exists(),
        "must not write global cursor config"
    );
}

#[test]
fn gemini_local_writes_project_settings() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    get_integration("gemini").unwrap().install(&ctx).unwrap();
    let settings = read_json(&proj.path().join(".gemini").join("settings.json"));
    assert!(settings["mcpServers"]["tokensave"].is_object());
    assert!(!home.path().join(".gemini").join("settings.json").exists());
}

#[test]
fn kiro_local_writes_workspace_mcp() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    get_integration("kiro").unwrap().install(&ctx).unwrap();
    let mcp = read_json(&proj.path().join(".kiro").join("settings").join("mcp.json"));
    assert!(mcp["mcpServers"]["tokensave"].is_object());
    assert!(!home
        .path()
        .join(".kiro")
        .join("settings")
        .join("mcp.json")
        .exists());
}

#[test]
fn zed_local_writes_project_settings() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    get_integration("zed").unwrap().install(&ctx).unwrap();
    let settings = read_json(&proj.path().join(".zed").join("settings.json"));
    assert!(settings["context_servers"]["tokensave"].is_object());
}

#[test]
fn roo_local_writes_project_mcp() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
        force_permission_style: false,
    };
    get_integration("roo-code").unwrap().install(&ctx).unwrap();
    let mcp = read_json(&proj.path().join(".roo").join("mcp.json"));
    assert!(mcp["mcpServers"]["tokensave"].is_object());
}
