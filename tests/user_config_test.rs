use tokensave::user_config::UserConfig;

#[test]
fn defaults_when_no_file() {
    let config = UserConfig::default();
    assert!(config.upload_enabled);
    assert_eq!(config.pending_upload, 0);
    assert_eq!(config.last_upload_at, 0);
    assert_eq!(config.last_worldwide_total, 0);
    assert_eq!(config.last_worldwide_fetch_at, 0);
    assert_eq!(config.last_flush_attempt_at, 0);
}

#[test]
fn round_trip_serialization() {
    let config = UserConfig {
        upload_enabled: false,
        pending_upload: 12345,
        last_upload_at: 1711375200,
        last_worldwide_total: 2847561,
        last_worldwide_fetch_at: 1711375200,
        last_flush_attempt_at: 1711375100,
        cached_latest_version: String::new(),
        last_version_check_at: 0,
        last_version_warning_at: 0,
        installed_agents: vec!["claude".to_string()],
        daemon_debounce: "30s".to_string(),
        cached_country_flags: Vec::new(),
        last_flags_fetch_at: 0,
        last_installed_version: "1.2.3".to_string(),
        last_pricing_fetch_at: 0,
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: UserConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.upload_enabled, false);
    assert_eq!(parsed.pending_upload, 12345);
    assert_eq!(parsed.last_worldwide_total, 2847561);
}

#[test]
fn missing_fields_use_defaults() {
    let toml_str = "upload_enabled = false\n";
    let parsed: UserConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(parsed.upload_enabled, false);
    assert_eq!(parsed.pending_upload, 0);
    assert_eq!(parsed.last_upload_at, 0);
}

#[test]
fn unknown_fields_ignored() {
    let toml_str = "upload_enabled = true\nsome_future_field = 42\n";
    let parsed: UserConfig = toml::from_str(toml_str).unwrap();
    assert!(parsed.upload_enabled);
}
