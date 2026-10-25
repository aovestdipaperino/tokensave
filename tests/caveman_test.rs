//! Filesystem-only fixtures: no process-global environment mutation.
use std::path::{Path, PathBuf};
use tokensave::caveman::{inspect, Activation, Installation};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
fn skill(root: &Path) -> PathBuf {
    let path = root.join("skills").join("caveman").join("SKILL.md");
    write(&path, "---\nname: caveman\n---\nPayload verbatim.\n");
    path
}
fn manifest(root: &Path, version: &str) {
    write(
        &root.join(".claude-plugin").join("plugin.json"),
        &serde_json::json!({"name":"caveman", "version":version}).to_string(),
    );
}

#[test]
fn absent_installation_does_not_infer_activation_from_mirror_or_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".claude");
    write(&root.join(".caveman-active"), "caveman");
    write(
        &tmp.path().join(".caveman.json"),
        r#"{"defaultMode":"caveman"}"#,
    );
    let r = inspect(tmp.path(), tmp.path(), "claude", None, None, true);
    assert_eq!(r.installation, Installation::Absent);
    assert_eq!(r.activation, Activation::Unknown);
    assert!(r.capabilities.is_empty());
}

#[test]
fn multiple_agents_discover_skills_without_claiming_session_activation() {
    for agent in [
        "claude", "codex", "cursor", "gemini", "opencode", "pi", "omp", "copilot",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("custom-agent-home");
        let file = skill(&root);
        let before = std::fs::read(&file).unwrap();
        let r = inspect(tmp.path(), tmp.path(), agent, Some(&root), None, true);
        assert_eq!(r.installation, Installation::Installed, "{agent}");
        assert_eq!(r.activation, Activation::Unknown);
        assert_eq!(r.version, None);
        assert_eq!(r.capabilities, ["conditional_mcp_instructions"]);
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
}

#[test]
fn verified_session_on_off_and_unrelated_windows() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".claude");
    skill(&root);
    manifest(&root, "4.0.1");
    let states = root.join(".caveman-sessions");
    write(&states.join("one.mode"), "caveman");
    write(&states.join("two.mode"), "off");
    write(&root.join(".caveman-active"), "ultracave");
    for (id, expected) in [
        (Some("one"), Activation::Active),
        (Some("two"), Activation::Inactive),
        (None, Activation::Unknown),
        (Some("missing"), Activation::Unknown),
    ] {
        let r = inspect(tmp.path(), tmp.path(), "claude", None, id, true);
        assert_eq!(r.activation, expected);
    }
    write(&states.join("one.mode"), "a".repeat(65).as_str());
    assert_eq!(
        inspect(tmp.path(), tmp.path(), "claude", None, Some("one"), true).activation,
        Activation::Unknown
    );
}

#[test]
fn unsupported_versions_and_malformed_metadata_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".claude");
    skill(&root);
    write(&root.join(".caveman-sessions").join("one.mode"), "caveman");
    for version in ["3.0.0", "4.0.2", "99.0.0"] {
        manifest(&root, version);
        let r = inspect(tmp.path(), tmp.path(), "claude", None, Some("one"), true);
        assert_eq!(r.installation, Installation::Unsupported);
        assert_eq!(r.activation, Activation::Unknown);
        assert!(r.capabilities.is_empty());
    }
    write(
        &root.join(".claude-plugin").join("plugin.json"),
        "{bad json",
    );
    let r = inspect(tmp.path(), tmp.path(), "claude", None, Some("one"), true);
    assert_eq!(r.installation, Installation::Partial);
    assert_eq!(r.activation, Activation::Unknown);
}

#[test]
fn opt_out_removal_and_malformed_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".claude");
    let file = skill(&root);
    let r = inspect(tmp.path(), tmp.path(), "claude", None, None, false);
    assert!(!r.enabled);
    assert!(r.capabilities.is_empty());
    write(&file, "not a skill");
    assert_eq!(
        inspect(tmp.path(), tmp.path(), "claude", None, None, true).installation,
        Installation::Partial
    );
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        inspect(tmp.path(), tmp.path(), "claude", None, None, true).installation,
        Installation::Absent
    );
}

#[test]
fn plugin_registry_discovery_and_conflicting_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".claude");
    let cache = tmp.path().join("plugin-cache");
    skill(&cache);
    manifest(&cache, "4.0.1");
    write(
        &root.join("plugins").join("installed_plugins.json"),
        &serde_json::json!({"version":2,"plugins":{"caveman@caveman":[{"installPath":cache}]}})
            .to_string(),
    );
    let r = inspect(tmp.path(), tmp.path(), "claude", None, None, true);
    assert_eq!(r.installation, Installation::Installed);
    assert_eq!(r.version.as_deref(), Some("4.0.1"));
    write(
        &cache.join(".claude-plugin").join("plugin.json"),
        r#"{"name":"another-plugin","version":"4.0.1"}"#,
    );
    assert_eq!(
        inspect(tmp.path(), tmp.path(), "claude", None, None, true).installation,
        Installation::Partial
    );
}

#[test]
fn invalid_session_id_and_unsupported_agent_are_diagnostic_only() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".claude");
    skill(&root);
    manifest(&root, "4.0.1");
    let r = inspect(
        tmp.path(),
        tmp.path(),
        "claude",
        None,
        Some("../other"),
        true,
    );
    assert_eq!(r.activation, Activation::Unknown);
    assert!(r.warnings.iter().any(|w| w.contains("Invalid session")));
    assert_eq!(
        inspect(tmp.path(), tmp.path(), "zed", None, None, true).installation,
        Installation::Unsupported
    );
}

#[test]
fn conflicting_startup_policy_and_malformed_configuration_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    skill(&tmp.path().join(".claude"));
    write(
        &tmp.path().join(".caveman.json"),
        r#"{"defaultMode":"unknown-mode"}"#,
    );
    let r = inspect(tmp.path(), tmp.path(), "claude", None, None, true);
    assert_eq!(r.installation, Installation::Unsupported);
    assert!(r.capabilities.is_empty());
    write(&tmp.path().join(".caveman.json"), "{malformed");
    let r = inspect(tmp.path(), tmp.path(), "claude", None, None, true);
    assert!(r
        .warnings
        .iter()
        .any(|w| w.contains("could not be interpreted")));
    assert_eq!(r.activation, Activation::Unknown);
    write(
        &tmp.path().join(".caveman.json"),
        "{ // policy only\n\"defaultMode\": \"manual\",\n}",
    );
    assert!(inspect(tmp.path(), tmp.path(), "claude", None, None, true)
        .warnings
        .iter()
        .any(|w| w.contains("off/manual")));
}

#[test]
fn known_delegation_overlap_is_reported_without_rewriting_instructions() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".cursor");
    skill(&root);
    let instructions = root.join("agents").join("cavecrew-investigator.md");
    let original = "Use this agent for research and broad repository exploration.";
    write(&instructions, original);
    let report = inspect(tmp.path(), tmp.path(), "cursor", None, None, true);
    assert!(report.warnings.iter().any(|w| w.contains("Cavecrew")));
    assert_eq!(std::fs::read_to_string(&instructions).unwrap(), original);
}

#[test]
fn doctor_emits_read_only_json_and_honors_opt_out() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["doctor", "--caveman", "--agent", "claude"])
        .env("HOME", tmp.path())
        .env("USERPROFILE", tmp.path())
        .env("CLAUDE_CONFIG_DIR", tmp.path().join(".claude"))
        .env("TOKENSAVE_CAVEMAN_COMPATIBILITY", "0")
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reports: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reports[0]["agent"], "claude");
    assert_eq!(reports[0]["enabled"], false);
    assert_eq!(reports[0]["installation"], "absent");
    assert_eq!(reports[0]["activation"], "unknown");
    assert!(!tmp.path().join(".claude").exists());
    assert!(!project.join(".tokensave").exists());
}
