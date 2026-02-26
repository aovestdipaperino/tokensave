use codegraph::config::*;
use tempfile::TempDir;

#[test]
fn test_default_config_has_rust_patterns() {
    let config = CodeGraphConfig::default();
    assert!(config.include.iter().any(|p| p == "**/*.rs"));
    assert!(config.exclude.iter().any(|p| p == "target/**"));
}

#[test]
fn test_save_and_load_config() {
    let dir = TempDir::new().unwrap();
    let config = CodeGraphConfig::default();
    save_config(dir.path(), &config).unwrap();
    let loaded = load_config(dir.path()).unwrap();
    assert_eq!(config.version, loaded.version);
    assert_eq!(config.include, loaded.include);
}

#[test]
fn test_should_include_file() {
    let config = CodeGraphConfig::default();
    assert!(should_include_file("src/main.rs", &config));
    assert!(!should_include_file("target/debug/foo", &config));
    assert!(!should_include_file("node_modules/foo.rs", &config));
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
