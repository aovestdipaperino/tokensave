use std::io::Write;
use tempfile::NamedTempFile;
use tokensave::sync::*;

#[test]
fn test_content_hash_deterministic() {
    let hash1 = content_hash("fn main() {}");
    let hash2 = content_hash("fn main() {}");
    assert_eq!(hash1, hash2);
}

#[test]
fn test_content_hash_different() {
    let hash1 = content_hash("fn main() {}");
    let hash2 = content_hash("fn main() { println!(\"hello\"); }");
    assert_ne!(hash1, hash2);
}

#[test]
fn test_file_stat_returns_mtime_and_size() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"hello world").unwrap();
    f.flush().unwrap();
    let (mtime, size) = file_stat(f.path()).unwrap();
    assert!(mtime > 0, "mtime should be positive");
    assert_eq!(size, 11);
}

#[test]
fn test_file_stat_nonexistent() {
    assert!(file_stat(std::path::Path::new("/nonexistent/file.rs")).is_none());
}

#[test]
fn test_read_source_file_utf8() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"fn main() {}").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "fn main() {}");
}

#[test]
fn test_read_source_file_utf8_bom() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"\xEF\xBB\xBFfn main() {}").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "fn main() {}");
}

#[test]
fn test_read_source_file_utf16_le() {
    let mut f = NamedTempFile::new().unwrap();
    // UTF-16 LE BOM + "hi"
    f.write_all(b"\xFF\xFE\x68\x00\x69\x00").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "hi");
}

#[test]
fn test_read_source_file_utf16_be() {
    let mut f = NamedTempFile::new().unwrap();
    // UTF-16 BE BOM + "hi"
    f.write_all(b"\xFE\xFF\x00\x68\x00\x69").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "hi");
}

#[test]
fn test_read_source_file_invalid_encoding() {
    let mut f = NamedTempFile::new().unwrap();
    // Invalid UTF-8 sequence without any BOM
    f.write_all(b"\x80\x81\x82\x83").unwrap();
    assert!(read_source_file(f.path()).is_err());
}
