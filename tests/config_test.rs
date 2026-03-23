use codegraph::config::*;
use tempfile::TempDir;

#[test]
fn test_default_config_has_exclude_patterns() {
    let config = CodeGraphConfig::default();
    assert!(config.exclude.iter().any(|p| p == "target/**"));
    assert!(config.exclude.iter().any(|p| p == ".git/**"));
}

#[test]
fn test_save_and_load_config() {
    let dir = TempDir::new().unwrap();
    let config = CodeGraphConfig::default();
    save_config(dir.path(), &config).unwrap();
    let loaded = load_config(dir.path()).unwrap();
    assert_eq!(config.version, loaded.version);
    assert_eq!(config.exclude, loaded.exclude);
}

#[test]
fn test_is_excluded() {
    let config = CodeGraphConfig::default();
    assert!(!is_excluded("src/main.rs", &config));
    assert!(is_excluded("target/debug/foo", &config));
    assert!(is_excluded("node_modules/foo.rs", &config));
    assert!(is_excluded("build/classes/App.class", &config));
}

#[test]
fn test_codegraph_dir_creation() {
    let dir = TempDir::new().unwrap();
    let cg_dir = get_codegraph_dir(dir.path());
    assert!(cg_dir.ends_with(".codegraph"));
}

#[test]
fn test_config_serde_roundtrip() {
    let config = CodeGraphConfig::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: CodeGraphConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.version, deserialized.version);
    assert_eq!(config.max_file_size, deserialized.max_file_size);
}

#[test]
fn test_legacy_config_with_include_field_still_loads() {
    let dir = TempDir::new().unwrap();
    let codegraph_dir = dir.path().join(".codegraph");
    std::fs::create_dir_all(&codegraph_dir).unwrap();
    // Simulate an old config that still has an "include" field
    let legacy_json = r#"{
        "version": 1,
        "root_dir": ".",
        "include": ["**/*.rs"],
        "exclude": ["target/**", ".git/**", ".codegraph/**"],
        "max_file_size": 1048576,
        "extract_docstrings": true,
        "track_call_sites": true,
        "enable_embeddings": false
    }"#;
    std::fs::write(codegraph_dir.join("config.json"), legacy_json).unwrap();
    let loaded = load_config(dir.path()).unwrap();
    assert_eq!(loaded.version, 1);
    assert!(loaded.exclude.contains(&"target/**".to_string()));
}
