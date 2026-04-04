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
