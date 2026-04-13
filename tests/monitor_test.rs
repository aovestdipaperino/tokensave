use std::path::Path;
use tempfile::TempDir;

/// Helper: write an entry to a specific mmap dir.
fn write(dir: &Path, project: &Path, prefix: &str, tool: &str, delta: u64, before: u64) {
    tokensave::monitor::write_entry_to(dir, project, prefix, tool, delta, before);
}

/// Helper: open reader at a specific mmap dir.
fn reader(dir: &Path) -> tokensave::monitor::MmapReader {
    tokensave::monitor::MmapReader::open_at(dir).unwrap()
}

#[test]
fn test_write_and_read_entry() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(dir.path(), project.path(), "tokensave", "tokensave_context", 63_102, 290_000);

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 1);

    let entry = r.entry(0).unwrap();
    assert_eq!(entry.prefix, "tokensave");
    assert_eq!(entry.tool_name, "tokensave_context");
    assert_eq!(entry.delta, 63_102);
    assert_eq!(entry.before, 290_000);
    assert!(entry.timestamp > 0);
    assert!(!entry.project.is_empty());
}

#[test]
fn test_ring_buffer_wraps() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    for i in 0..260u64 {
        write(dir.path(), project.path(), "tokensave", "tokensave_search", i + 1, i * 10);
    }

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 260);

    // Slot 0 should now have entry 256 (index 256, delta=257)
    let entry = r.entry(0).unwrap();
    assert_eq!(entry.delta, 257);

    // Slot 3 should have entry 259 (index 259, delta=260)
    let entry = r.entry(3).unwrap();
    assert_eq!(entry.delta, 260);
}

#[test]
fn test_write_entry_accumulates() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(dir.path(), project.path(), "tokensave", "tokensave_context", 100, 500);
    write(dir.path(), project.path(), "tokensave", "tokensave_search", 50, 200);

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 2);

    let e0 = r.entry(0).unwrap();
    let e1 = r.entry(1).unwrap();
    assert_eq!(e0.delta + e1.delta, 150);
}

#[test]
fn test_entry_label_format() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(dir.path(), project.path(), "tokensave", "tokensave_context", 42, 100);

    let r = reader(dir.path());
    let entry = r.entry(0).unwrap();
    let label = entry.label();
    assert!(label.starts_with("tokensave - "), "got: {label}");
    assert!(label.ends_with(" - tokensave_context"), "got: {label}");
}

#[test]
fn test_tool_name_truncation() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    let long_name = "a".repeat(100);
    write(dir.path(), project.path(), "tokensave", &long_name, 42, 100);

    let r = reader(dir.path());
    let entry = r.entry(0).unwrap();
    // Should be truncated to 31 chars (32 bytes with null)
    assert_eq!(entry.tool_name.len(), 31);
}

#[test]
fn test_multiple_projects() {
    let dir = TempDir::new().unwrap();
    let project_a = TempDir::with_prefix("alpha").unwrap();
    let project_b = TempDir::with_prefix("bravo").unwrap();

    write(dir.path(), project_a.path(), "tokensave", "tokensave_context", 100, 500);
    write(dir.path(), project_b.path(), "tokensave", "tokensave_search", 200, 600);

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 2);

    let e0 = r.entry(0).unwrap();
    let e1 = r.entry(1).unwrap();
    assert_ne!(e0.project, e1.project);
}

#[test]
fn test_different_prefixes() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(dir.path(), project.path(), "tokensave", "tokensave_context", 100, 500);
    write(dir.path(), project.path(), "othertool", "do_stuff", 200, 600);

    let r = reader(dir.path());
    let e0 = r.entry(0).unwrap();
    let e1 = r.entry(1).unwrap();
    assert_eq!(e0.prefix, "tokensave");
    assert_eq!(e1.prefix, "othertool");
}
