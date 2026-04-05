#[test]
fn worker_response_deserializes() {
    #[derive(serde::Deserialize)]
    struct WorkerResponse { total: u64 }
    let json = r#"{"total": 2847561}"#;
    let parsed: WorkerResponse = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.total, 2847561);
}

#[test]
fn increment_request_body_format() {
    let amount: u64 = 4823;
    let body = serde_json::json!({ "amount": amount });
    assert_eq!(body["amount"], 4823);
}

#[test]
fn is_newer_version_stable_comparisons() {
    assert!(tokensave::cloud::is_newer_version("2.3.0", "2.4.0"));
    assert!(tokensave::cloud::is_newer_version("2.4.0", "3.0.0"));
    assert!(!tokensave::cloud::is_newer_version("2.4.0", "2.4.0"));
    assert!(!tokensave::cloud::is_newer_version("2.4.0", "2.3.0"));
}

#[test]
fn is_newer_version_beta_comparisons() {
    // Cross-channel comparisons always return false (separate update channels)
    assert!(!tokensave::cloud::is_newer_version("2.5.0-beta.1", "2.5.0"));
    assert!(!tokensave::cloud::is_newer_version("2.5.0", "2.5.0-beta.1"));
    assert!(!tokensave::cloud::is_newer_version("2.5.0-beta.1", "2.6.0"));
    assert!(!tokensave::cloud::is_newer_version("2.6.0", "2.5.0-beta.1"));
    // Same-channel beta comparisons still work
    assert!(tokensave::cloud::is_newer_version("2.5.0-beta.1", "2.5.0-beta.2"));
    assert!(!tokensave::cloud::is_newer_version("2.5.0-beta.2", "2.5.0-beta.1"));
    assert!(tokensave::cloud::is_newer_version("2.5.0-beta.1", "2.6.0-beta.1"));
}

#[test]
fn is_newer_minor_version_ignores_patch_bumps() {
    // Patch-only bump → not a minor update
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.0", "3.2.1"));
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.1", "3.2.2"));
    // Minor bump → yes
    assert!(tokensave::cloud::is_newer_minor_version("3.2.1", "3.3.0"));
    assert!(tokensave::cloud::is_newer_minor_version("3.2.0", "3.3.0"));
    // Major bump → yes
    assert!(tokensave::cloud::is_newer_minor_version("3.2.1", "4.0.0"));
    // Same version → no
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.0", "3.2.0"));
    // Older version → no
    assert!(!tokensave::cloud::is_newer_minor_version("3.3.0", "3.2.1"));
}

#[test]
fn is_newer_minor_version_beta() {
    // Cross-channel: always false regardless of version distance
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.0-beta.1", "3.2.0"));
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.0-beta.1", "3.3.0"));
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.0", "3.3.0-beta.1"));
    // Same-channel beta: minor bump detected
    assert!(tokensave::cloud::is_newer_minor_version("3.2.0-beta.1", "3.3.0-beta.1"));
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.0-beta.1", "3.2.0-beta.2"));
}

#[test]
fn is_newer_version_same_version() {
    assert!(!tokensave::cloud::is_newer_version("3.2.1", "3.2.1"));
}

#[test]
fn is_newer_version_all_components() {
    // Latest is newer in each component
    assert!(tokensave::cloud::is_newer_version("3.2.1", "3.3.0"));
    assert!(tokensave::cloud::is_newer_version("3.2.1", "4.0.0"));
    assert!(tokensave::cloud::is_newer_version("3.2.1", "3.2.2"));
    // Latest is older
    assert!(!tokensave::cloud::is_newer_version("3.3.0", "3.2.1"));
}

#[test]
fn is_newer_version_cross_channel_blocked() {
    // Beta vs stable (cross-channel = false)
    assert!(!tokensave::cloud::is_newer_version("3.2.1", "3.3.0-beta.1"));
    assert!(!tokensave::cloud::is_newer_version("3.2.1-beta.1", "3.3.0"));
}

#[test]
fn is_newer_version_beta_ordering() {
    assert!(tokensave::cloud::is_newer_version("3.2.1-beta.1", "3.2.1-beta.2"));
    assert!(!tokensave::cloud::is_newer_version("3.2.1-beta.2", "3.2.1-beta.1"));
}

#[test]
fn is_newer_version_invalid_versions() {
    assert!(!tokensave::cloud::is_newer_version("invalid", "3.2.1"));
    assert!(!tokensave::cloud::is_newer_version("3.2.1", "invalid"));
}

#[test]
fn is_newer_minor_version_patch_only() {
    // Patch-only bump returns false
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.1", "3.2.2"));
}

#[test]
fn is_newer_minor_version_minor_bump() {
    assert!(tokensave::cloud::is_newer_minor_version("3.2.1", "3.3.0"));
}

#[test]
fn is_newer_minor_version_major_bump() {
    assert!(tokensave::cloud::is_newer_minor_version("3.2.1", "4.0.0"));
}

#[test]
fn is_newer_minor_version_same() {
    assert!(!tokensave::cloud::is_newer_minor_version("3.2.1", "3.2.1"));
}

#[test]
fn is_beta_returns_bool() {
    // Just verify it returns a bool and doesn't panic
    let _ = tokensave::cloud::is_beta();
}

#[test]
fn upgrade_command_cargo() {
    use tokensave::cloud::{upgrade_command, InstallMethod};
    let cmd = upgrade_command(&InstallMethod::Cargo);
    assert!(cmd.contains("cargo install"));
}

#[test]
fn upgrade_command_brew() {
    use tokensave::cloud::{upgrade_command, InstallMethod};
    let cmd = upgrade_command(&InstallMethod::Brew);
    assert!(cmd.contains("brew"));
}

#[test]
fn upgrade_command_scoop() {
    use tokensave::cloud::{upgrade_command, InstallMethod};
    let cmd = upgrade_command(&InstallMethod::Scoop);
    assert!(cmd.contains("scoop"));
}

#[test]
fn upgrade_command_unknown() {
    use tokensave::cloud::{upgrade_command, InstallMethod};
    let cmd = upgrade_command(&InstallMethod::Unknown);
    assert!(cmd.contains("cargo install"));
}

#[test]
fn detect_install_method_no_panic() {
    // Just verify it returns without panic
    let _ = tokensave::cloud::detect_install_method();
}
