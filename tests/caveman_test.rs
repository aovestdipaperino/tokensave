//! Caveman cooperation (#797). Discovery is exercised through
//! `caveman::inspect` with an explicit `Scope`, so no test mutates the
//! process environment; the CLI test sets the child's environment only.

use std::path::{Path, PathBuf};
use std::process::Command;

use tokensave::caveman::{
    self, inspect, Activation, Installation, Scope, COOPERATION_INSTRUCTIONS,
};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    std::fs::write(path, text).expect("write fixture");
}

fn skill(base: &Path) -> PathBuf {
    let path = base.join("skills").join("caveman").join("SKILL.md");
    write(&path, "---\nname: caveman\ndescription: test\n---\nBody.\n");
    path
}

fn manifest(base: &Path, version: &str) {
    write(
        &base.join(".claude-plugin").join("plugin.json"),
        &serde_json::json!({ "name": "caveman", "version": version }).to_string(),
    );
}

/// A home that is also the project, with Caveman's default user config.
fn scope(home: &Path) -> Scope {
    Scope::new(home, home)
}

#[test]
fn absent_installation_ignores_the_mirror_flag_and_startup_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write(
        &tmp.path().join(".claude").join(".caveman-active"),
        "caveman",
    );
    write(
        &tmp.path().join(".caveman.json"),
        r#"{"defaultMode":"caveman"}"#,
    );
    let r = inspect(&scope(tmp.path()), "claude", None, true);
    assert_eq!(r.installation, Installation::Absent);
    assert_eq!(r.activation, Activation::Unknown);
    assert!(r.capabilities.is_empty());
}

#[test]
fn every_agent_discovers_a_skill_without_claiming_activation_or_writing() {
    for agent in tokensave::caveman::AGENTS {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("agent-home");
        let file = skill(&root);
        let before = std::fs::read(&file).expect("read skill");
        let mut s = scope(tmp.path());
        s.agent_root = Some(root.clone());
        let r = inspect(&s, agent, None, true);
        assert_eq!(r.installation, Installation::Installed, "{agent}");
        assert_eq!(r.activation, Activation::Unknown, "{agent}");
        assert_eq!(r.version, None, "{agent}");
        assert_eq!(r.capabilities, ["conditional_mcp_instructions"], "{agent}");
        assert_eq!(std::fs::read(&file).expect("reread"), before, "{agent}");
    }
}

#[test]
fn project_and_shared_skill_locations_are_found() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&home).expect("home");
    let local = skill(&project.join(".agents"));
    let r = inspect(&Scope::new(&home, &project), "codex", None, true);
    assert_eq!(r.skill.as_deref(), Some(local.as_path()));

    std::fs::remove_dir_all(&project).expect("remove project");
    std::fs::create_dir_all(&project).expect("project");
    let shared = skill(&home.join(".agents"));
    let r = inspect(&Scope::new(&home, &project), "gemini", None, true);
    assert_eq!(r.skill.as_deref(), Some(shared.as_path()));
}

#[test]
fn session_state_is_read_only_for_the_named_session_of_a_verified_release() {
    for version in tokensave::caveman::VERIFIED_RELEASES {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join(".claude");
        skill(&root);
        manifest(&root, version);
        let sessions = root.join(".caveman-sessions");
        write(&sessions.join("one.mode"), "caveman\n");
        write(&sessions.join("two.mode"), "off");
        write(&sessions.join("legacy.mode"), "wenyan-lite");
        write(&root.join(".caveman-active"), "ultracave");
        for (id, expected) in [
            (Some("one"), Activation::Active),
            (Some("two"), Activation::Inactive),
            (Some("legacy"), Activation::Active),
            (None, Activation::Unknown),
            (Some("missing"), Activation::Unknown),
        ] {
            let r = inspect(&scope(tmp.path()), "claude", id, true);
            assert!(r.verified_release, "{version}");
            assert_eq!(r.activation, expected, "{version} {id:?}");
        }
        write(&sessions.join("one.mode"), &"a".repeat(65));
        let r = inspect(&scope(tmp.path()), "claude", Some("one"), true);
        assert_eq!(r.activation, Activation::Unknown, "oversized state file");
    }
}

#[test]
fn unverified_release_keeps_cooperation_but_not_session_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".claude");
    skill(&root);
    write(&root.join(".caveman-sessions").join("one.mode"), "caveman");
    for version in ["3.0.0", "4.0.2", "99.0.0"] {
        manifest(&root, version);
        let r = inspect(&scope(tmp.path()), "claude", Some("one"), true);
        assert_eq!(r.installation, Installation::Installed, "{version}");
        assert!(!r.verified_release, "{version}");
        assert_eq!(r.version.as_deref(), Some(version));
        assert_eq!(r.activation, Activation::Unknown, "{version}");
        assert_eq!(r.capabilities, ["conditional_mcp_instructions"]);
    }
}

#[test]
fn malformed_or_foreign_metadata_fails_closed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".claude");
    skill(&root);
    write(&root.join(".caveman-sessions").join("one.mode"), "caveman");
    for body in [
        "{bad json",
        r#"{"name":"another-plugin","version":"4.1.0"}"#,
    ] {
        write(&root.join(".claude-plugin").join("plugin.json"), body);
        let r = inspect(&scope(tmp.path()), "claude", Some("one"), true);
        assert_eq!(r.installation, Installation::Partial, "{body}");
        assert_eq!(r.activation, Activation::Unknown, "{body}");
        assert!(r.capabilities.is_empty(), "{body}");
    }
}

#[test]
fn opt_out_malformed_skill_and_removal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".claude");
    let file = skill(&root);
    let r = inspect(&scope(tmp.path()), "claude", None, false);
    assert!(!r.enabled);
    assert_eq!(r.installation, Installation::Installed);
    assert!(r.capabilities.is_empty(), "opted out: no instructions");

    write(&file, "not a skill");
    let r = inspect(&scope(tmp.path()), "claude", None, true);
    assert_eq!(r.installation, Installation::Partial);
    assert!(r.capabilities.is_empty());

    write(&file, &format!("name: caveman\n{}", "x".repeat(64 * 1024)));
    let r = inspect(&scope(tmp.path()), "claude", None, true);
    assert_eq!(r.installation, Installation::Partial, "oversized skill");

    std::fs::remove_dir_all(&root).expect("remove");
    let r = inspect(&scope(tmp.path()), "claude", None, true);
    assert_eq!(r.installation, Installation::Absent);
}

#[test]
fn plugin_registry_points_at_the_installed_plugin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".claude");
    let cache = tmp.path().join("plugin-cache");
    skill(&cache);
    manifest(&cache, "4.1.0");
    write(
        &root.join("plugins").join("installed_plugins.json"),
        &serde_json::json!({
            "version": 2,
            "plugins": { "caveman@caveman": [{ "installPath": cache }] }
        })
        .to_string(),
    );
    let r = inspect(&scope(tmp.path()), "claude", None, true);
    assert_eq!(r.installation, Installation::Installed);
    assert_eq!(r.version.as_deref(), Some("4.1.0"));
    assert!(r.verified_release);
    assert!(r.capabilities.contains(&"session_state_diagnostics"));

    write(
        &root.join("settings.json"),
        r#"{ "enabledPlugins": { "caveman@caveman": false } }"#,
    );
    let r = inspect(&scope(tmp.path()), "claude", None, true);
    assert!(r.warnings.iter().any(|w| w.contains("disabled")));
}

#[test]
fn invalid_session_id_and_unknown_agent_are_diagnostic_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".claude");
    skill(&root);
    manifest(&root, "4.1.0");
    write(&tmp.path().join("other.mode"), "caveman");
    let long = "a".repeat(129);
    for id in ["../other", "", long.as_str()] {
        let r = inspect(&scope(tmp.path()), "claude", Some(id), true);
        assert_eq!(r.activation, Activation::Unknown, "{id}");
        assert!(r.warnings.iter().any(|w| w.contains("Invalid session")));
    }
    let r = inspect(&scope(tmp.path()), "zed", None, true);
    assert_eq!(r.installation, Installation::Unsupported);
    assert!(r.capabilities.is_empty());
}

#[test]
fn startup_policy_is_reported_but_never_changes_installation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let project = home.join("work").join("repo");
    std::fs::create_dir_all(&project).expect("project");
    skill(&home.join(".claude"));
    let s = Scope::new(&home, &project);

    // The nearest repository config wins over the user config.
    write(
        &home.join(".config").join("caveman").join("config.json"),
        r#"{"defaultMode":"caveman"}"#,
    );
    write(
        &home.join("work").join(".caveman.json"),
        "{ // policy only\n\"defaultMode\": \"manual\",\n}",
    );
    let r = inspect(&s, "claude", None, true);
    assert_eq!(r.installation, Installation::Installed);
    assert!(r.warnings.iter().any(|w| w.contains("off or manual")));

    write(
        &home.join("work").join(".caveman.json"),
        r#"{"defaultMode":"unknown-mode"}"#,
    );
    let r = inspect(&s, "claude", None, true);
    assert_eq!(r.installation, Installation::Installed);
    assert!(r.warnings.iter().any(|w| w.contains("not a mode")));
    assert_eq!(r.activation, Activation::Unknown);
}

#[test]
fn cavecrew_overlap_is_reported_without_rewriting_instructions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".cursor");
    skill(&root);
    let agent = root.join("agents").join("cavecrew-investigator.md");
    let original = "Use this agent for broad repository exploration.";
    write(&agent, original);
    let r = inspect(&scope(tmp.path()), "cursor", None, true);
    assert!(r.warnings.iter().any(|w| w.contains("cavecrew")));
    assert_eq!(std::fs::read_to_string(&agent).expect("read"), original);
}

#[test]
fn cooperation_instructions_keep_results_exact_and_defer_to_the_user() {
    for phrase in [
        "indexed project",
        "tokensave_status",
        "never compress or paraphrase",
        "errors",
        "User instructions take precedence",
    ] {
        assert!(COOPERATION_INSTRUCTIONS.contains(phrase), "{phrase}");
    }
}

/// `doctor --caveman` adds its section to the normal checks (stderr) and
/// prints the JSON report on stdout, without creating agent or index state.
#[test]
fn doctor_caveman_runs_the_normal_checks_and_prints_json() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    let project = home.join("project");
    std::fs::create_dir_all(&project).expect("project");
    let claude = home.join(".claude");
    skill(&claude);
    manifest(&claude, "4.1.0");
    write(&claude.join(".caveman-sessions").join("abc.mode"), "off");
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args([
            "doctor",
            "--agent",
            "claude",
            "--caveman",
            "--session-id",
            "abc",
        ])
        .current_dir(&project)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("CLAUDE_CONFIG_DIR", &claude)
        .env("TOKENSAVE_CAVEMAN_COMPATIBILITY", "1")
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .output()
        .expect("run tokensave");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Current project"),
        "normal checks ran: {stderr}"
    );
    assert!(stderr.contains("Caveman compatibility"), "{stderr}");
    let reports: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is the JSON report");
    assert_eq!(reports[0]["agent"], "claude");
    assert_eq!(reports[0]["enabled"], true);
    assert_eq!(reports[0]["installation"], "installed");
    assert_eq!(reports[0]["version"], "4.1.0");
    assert_eq!(reports[0]["activation"], "inactive");
    assert!(!project.join(".tokensave").exists());
}

#[test]
fn init_notice_points_to_the_opt_in_without_enabling_it() {
    assert_eq!(caveman::init_notice_text(&[], false), None);
    let off = caveman::init_notice_text(&["claude", "codex"], false).expect("notice");
    assert!(off.contains("claude, codex"), "{off}");
    assert!(off.contains("caveman_compatibility = true"), "{off}");
    assert!(off.contains(caveman::COMPATIBILITY_ENV), "{off}");
    let on = caveman::init_notice_text(&["claude"], true).expect("notice");
    assert!(on.contains("cooperation is on"), "{on}");
    let missing = caveman::init_notice_text(&[], true).expect("notice");
    assert!(missing.contains("no Caveman skill was found"), "{missing}");
}

/// Runs `tokensave init` on a fresh project with an isolated home.
fn run_init(home: &Path, project: &Path, compatibility: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["init", "--no-git-hook", "--cpu-usage", "balanced"])
        .arg(project)
        .current_dir(project)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("TOKENSAVE_CAVEMAN_COMPATIBILITY", compatibility)
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .env("TOKENSAVE_UPDATE_CHECK", "off")
        .output()
        .expect("run tokensave");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `init` reports an installed Caveman and leaves the opt-in setting alone.
#[test]
fn init_reports_an_installed_caveman() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    let project = home.join("project");
    write(&project.join("lib.rs"), "fn main() {}\n");
    let skill_path = skill(&home.join(".claude"));
    let before = std::fs::read(&skill_path).expect("skill");

    let stderr = run_init(home, &project, "0");
    assert!(stderr.contains("Caveman found for claude"), "{stderr}");
    assert!(stderr.contains("caveman_compatibility = true"), "{stderr}");
    assert_eq!(std::fs::read(&skill_path).expect("skill"), before);
    let config = home.join(".tokensave").join("config.toml");
    let saved = std::fs::read_to_string(&config).unwrap_or_default();
    assert!(!saved.contains("caveman_compatibility"), "{saved}");
}

#[test]
fn init_says_nothing_about_caveman_when_it_is_absent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    let project = home.join("project");
    write(&project.join("lib.rs"), "fn main() {}\n");
    let stderr = run_init(home, &project, "0");
    assert!(!stderr.contains("Caveman"), "{stderr}");
}
