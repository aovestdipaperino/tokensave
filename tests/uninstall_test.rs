//! CLI integration cleanup in disposable homes and projects.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, text).expect("write fixture");
}

fn uninstall(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("TOKENSAVE_MANAGE_RULES", "1")
        .env("TOKENSAVE_MANAGE_PERMISSIONS", "1")
        .args(["uninstall"])
        .args(args)
        .output()
        .expect("run uninstall")
}

#[test]
fn targeted_dry_run_and_apply_keep_shared_toml_and_user_settings() {
    let home = tempfile::tempdir().expect("home");
    let home = home.path();
    let config = home.join(".codex").join("config.toml");
    let state = home.join(".tokensave");
    let malformed_user_settings = state.join("config.toml");
    let original = "# User preferences\nmodel = 'test'\n\n[mcp_servers.tokensave]\ncommand = 'tokensave'\nargs = ['serve']\n\n[mcp_servers.other]\ncommand = 'other'\n";
    write(&config, original);
    write(&malformed_user_settings, "[unterminated");

    let preview = uninstall(home, home, &["--agent", "codex", "--dry-run"]);
    let text = String::from_utf8_lossy(&preview.stderr);
    assert!(!preview.status.success(), "{text}");
    assert!(
        text.contains("Plan: edit") && text.contains("config.toml"),
        "{text}"
    );
    assert!(
        text.contains("invalid TOML") && text.contains("executable is still installed"),
        "{text}"
    );
    assert_eq!(fs::read_to_string(&config).expect("config"), original);
    assert_eq!(
        fs::read_to_string(&malformed_user_settings).expect("settings"),
        "[unterminated"
    );
    assert_eq!(fs::read_dir(&state).expect("state directory").count(), 1);
    assert_eq!(
        fs::read_dir(config.parent().expect("config parent"))
            .expect("config dir")
            .count(),
        1
    );

    let result = uninstall(home, home, &["--agent", "codex", "--verbose"]);
    let text = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{text}");
    assert!(
        text.contains("Processing") && text.contains("1 failed"),
        "{text}"
    );
    let remaining = fs::read_to_string(&config).expect("updated config");
    assert!(remaining.contains("# User preferences") && remaining.contains("[mcp_servers.other]"));
    assert!(!remaining.contains("mcp_servers.tokensave"));
    assert_eq!(
        fs::read_to_string(&malformed_user_settings).expect("settings"),
        "[unterminated"
    );
}

#[test]
fn local_cleanup_is_idempotent_and_keeps_global_and_foreign_content() {
    let home = tempfile::tempdir().expect("home");
    let project = tempfile::tempdir().expect("project");
    let global = home.path().join(".cursor").join("mcp.json");
    let local = project.path().join(".cursor").join("mcp.json");
    let hooks = project.path().join(".cursor").join("hooks.json");
    let mcp = r#"{"mcpServers":{"tokensave":{"command":"tokensave","args":["serve"]},"other":{"command":"other"}}}"#;
    write(&global, mcp);
    write(&local, mcp);
    write(
        &hooks,
        r#"{"hooks":{"stop":[{"command":"tokensave hook-stop"},{"command":"echo tokensave"}]}}"#,
    );
    let preview = uninstall(
        home.path(),
        project.path(),
        &["--agent", "cursor", "--local", "--dry-run"],
    );
    let text = String::from_utf8_lossy(&preview.stderr);
    assert!(
        preview.status.success() && text.contains("2 planned"),
        "{text}"
    );
    assert!(
        text.contains("mcp.json") && text.contains("hooks.json"),
        "{text}"
    );
    assert_eq!(fs::read_to_string(&local).expect("local config"), mcp);
    assert!(fs::read_to_string(&hooks)
        .expect("previewed hooks")
        .contains("tokensave hook-stop"));
    assert!(!home.path().join(".tokensave").exists());

    let result = uninstall(
        home.path(),
        project.path(),
        &["--agent", "cursor", "--local"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let remaining: serde_json::Value =
        serde_json::from_slice(&fs::read(local).expect("local config")).expect("JSON");
    assert!(remaining["mcpServers"].get("tokensave").is_none());
    assert_eq!(remaining["mcpServers"]["other"]["command"], "other");
    assert_eq!(fs::read_to_string(global).expect("global config"), mcp);
    assert!(!home.path().join(".tokensave").exists());
    assert!(fs::read_to_string(hooks)
        .expect("hooks")
        .contains("echo tokensave"));
    let repeat = uninstall(
        home.path(),
        project.path(),
        &["--agent", "cursor", "--local", "--quiet"],
    );
    let text = String::from_utf8_lossy(&repeat.stderr);
    assert!(
        repeat.status.success() && !text.contains("Summary"),
        "{text}"
    );
    assert!(text.contains("executable is still installed"), "{text}");
}

#[test]
fn global_cleanup_keeps_foreign_hooks_and_reports_partial_failure() {
    let home = tempfile::tempdir().expect("home");
    let home = home.path();
    let settings = home.join(".tokensave").join("state.toml");
    let hooks = home.join(".config").join("git").join("hooks");
    let owned = hooks.join("post-commit");
    let foreign = hooks.join("pre-commit");
    let agent = home.join(".cursor").join("mcp.json");
    write(&settings, "installed_agents = ['cursor']\n");
    write(&agent, "{not JSON");
    write(&foreign, "#!/bin/sh\necho foreign\n");
    write(&owned, "#!/bin/sh\n# tokensave: auto-sync\ntokensave sync >/dev/null 2>&1 &\n# tokensave: end auto-sync\necho other\n");

    let kept = uninstall(home, home, &["--keep-git-hooks"]);
    assert!(!kept.status.success());
    assert!(fs::read_to_string(&owned)
        .expect("hook")
        .contains("tokensave sync"));
    let removed = uninstall(home, home, &["--quiet"]);
    let text = String::from_utf8_lossy(&removed.stderr);
    assert!(
        !removed.status.success() && text.contains("invalid JSON"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(owned).expect("updated hook"),
        "#!/bin/sh\necho other\n"
    );
    assert_eq!(
        fs::read_to_string(foreign).expect("foreign hook"),
        "#!/bin/sh\necho foreign\n"
    );
    assert_eq!(fs::read_to_string(agent).expect("malformed"), "{not JSON");
    assert!(fs::read_to_string(settings)
        .expect("state")
        .contains("cursor"));
}

#[test]
fn targeted_agent_preserves_another_agents_rules_and_jsonc_comments() {
    let home = tempfile::tempdir().expect("home");
    let project = tempfile::tempdir().expect("project");
    let config = project.path().join("opencode.json");
    let rules = project.path().join("AGENTS.md");
    let other_block = "<!-- tokensave rules begin (agent: droid, version: test) -->\nDroid rules\n<!-- tokensave rules end -->\n";
    write(
        &config,
        "{\n  // user's comment\n  \"mcp\": {\n    \"tokensave\": {\"command\": [\"tokensave\", \"serve\"]},\n    \"other\": {\"command\": [\"other\"]},\n  },\n}\n",
    );
    write(
        &rules,
        &format!(
            "User instructions\n{other_block}<!-- tokensave rules begin (agent: opencode, version: test) -->\nOpenCode rules\n<!-- tokensave rules end -->\n"
        ),
    );

    let result = uninstall(
        home.path(),
        project.path(),
        &["--local", "--agent", "opencode"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let jsonc = fs::read_to_string(config).expect("shared JSONC");
    assert!(jsonc.contains("// user's comment") && jsonc.contains("\"other\""));
    assert!(!jsonc.contains("\"tokensave\""));
    assert_eq!(
        fs::read_to_string(rules).expect("shared rules"),
        format!("User instructions\n{other_block}")
    );
}

#[cfg(unix)]
#[test]
fn local_removal_skips_config_through_a_directory_link_outside_the_project() {
    let home = tempfile::tempdir().expect("home");
    let project = tempfile::tempdir().expect("project");
    let external = home.path().join(".cursor");
    let file = external.join("mcp.json");
    let content = r#"{"mcpServers":{"tokensave":{"command":"tokensave"}}}"#;
    write(&file, content);
    std::os::unix::fs::symlink(&external, project.path().join(".cursor")).expect("directory link");
    let result = uninstall(
        home.path(),
        project.path(),
        &["--local", "--agent", "cursor"],
    );
    let text = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success() && text.contains("outside the project"),
        "{text}"
    );
    assert_eq!(fs::read_to_string(file).expect("external config"), content);
}

#[test]
fn removing_all_global_hooks_releases_tokensaves_hookspath() {
    let home = tempfile::tempdir().expect("home");
    let home = home.path();
    let hooks = home.join(".config").join("git").join("hooks");
    let gitconfig = home.join(".gitconfig");
    let original = format!(
        "[core]\n\thooksPath = \"{}\"\n[user]\n\tname = Test\n",
        hooks.to_string_lossy().replace('\\', "/")
    );
    write(&gitconfig, &original);
    let hook = hooks.join("post-commit");
    write(&hook, "#!/bin/sh\n# tokensave: auto-sync\ntokensave sync >/dev/null 2>&1 &\n# tokensave: end auto-sync\n");

    let preview = uninstall(home, home, &["--dry-run"]);
    let text = String::from_utf8_lossy(&preview.stderr);
    assert!(
        preview.status.success() && text.contains("Plan: edit Git core.hooksPath"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(&gitconfig).expect("git config"),
        original
    );
    assert!(hook.is_file());

    let result = uninstall(home, home, &[]);
    let text = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{text}");
    assert!(!hook.exists());
    assert_eq!(
        fs::read_to_string(gitconfig).expect("git config"),
        "[core]\n[user]\n\tname = Test\n"
    );
}
