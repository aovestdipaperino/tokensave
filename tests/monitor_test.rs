use tempfile::TempDir;

#[test]
fn test_write_and_read_entry() {
    let dir = TempDir::new().unwrap();
    let project_root = dir.path();

    // Create the .tokensave dir
    std::fs::create_dir_all(project_root.join(".tokensave")).unwrap();

    // Write an entry
    tokensave::monitor::write_entry(project_root, "tokensave_context", 63_102, 290_000);

    // Read it back via the public reader
    let reader = tokensave::monitor::MmapReader::open(project_root).unwrap();
    assert_eq!(reader.write_idx(), 1);
    assert_eq!(reader.total_saved(), 63_102);

    let entry = reader.entry(0).unwrap();
    assert_eq!(entry.tool_name, "tokensave_context");
    assert_eq!(entry.delta, 63_102);
    assert_eq!(entry.before, 290_000);
    assert!(entry.timestamp > 0);
}

#[test]
fn test_ring_buffer_wraps() {
    let dir = TempDir::new().unwrap();
    let project_root = dir.path();
    std::fs::create_dir_all(project_root.join(".tokensave")).unwrap();

    // Write 260 entries (wraps around 256-entry ring)
    for i in 0..260u64 {
        tokensave::monitor::write_entry(project_root, "tokensave_search", i + 1, i * 10);
    }

    let reader = tokensave::monitor::MmapReader::open(project_root).unwrap();
    assert_eq!(reader.write_idx(), 260);

    // Slot 0 should now have entry 256 (index 256, delta=257)
    let entry = reader.entry(0).unwrap();
    assert_eq!(entry.delta, 257);

    // Slot 3 should have entry 259 (index 259, delta=260)
    let entry = reader.entry(3).unwrap();
    assert_eq!(entry.delta, 260);
}

#[test]
fn test_write_entry_noop_without_tokensave_dir() {
    let dir = TempDir::new().unwrap();
    // No .tokensave dir — write_entry should silently return
    tokensave::monitor::write_entry(dir.path(), "tokensave_context", 100, 200);
    // No panic, no error — just a no-op
}

#[test]
fn test_tool_name_truncation() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".tokensave")).unwrap();

    let long_name = "a".repeat(100);
    tokensave::monitor::write_entry(dir.path(), &long_name, 42, 100);

    let reader = tokensave::monitor::MmapReader::open(dir.path()).unwrap();
    let entry = reader.entry(0).unwrap();
    // Should be truncated to 63 chars (64 bytes with null)
    assert_eq!(entry.tool_name.len(), 63);
}
