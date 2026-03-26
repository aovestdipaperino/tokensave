use tokensave::display::{format_bytes, format_number, format_token_count};

// ── format_token_count ──────────────────────────────────────────────────────

#[test]
fn test_format_token_count_zero() {
    assert_eq!(format_token_count(0), "0");
}

#[test]
fn test_format_token_count_small() {
    assert_eq!(format_token_count(42), "42");
    assert_eq!(format_token_count(999), "999");
}

#[test]
fn test_format_token_count_thousands() {
    assert_eq!(format_token_count(1_000), "1.0k");
    assert_eq!(format_token_count(1_500), "1.5k");
    assert_eq!(format_token_count(45_300), "45.3k");
    assert_eq!(format_token_count(999_999), "1000.0k");
}

#[test]
fn test_format_token_count_millions() {
    assert_eq!(format_token_count(1_000_000), "1.0M");
    assert_eq!(format_token_count(1_200_000), "1.2M");
    assert_eq!(format_token_count(123_456_789), "123.5M");
}

// ── format_bytes ────────────────────────────────────────────────────────────

#[test]
fn test_format_bytes_small() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn test_format_bytes_kilobytes() {
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1_536), "1.5 KB");
    assert_eq!(format_bytes(1_048_575), "1024.0 KB");
}

#[test]
fn test_format_bytes_megabytes() {
    assert_eq!(format_bytes(1_048_576), "1.0 MB");
    assert_eq!(format_bytes(838_860_800), "800.0 MB");
}

#[test]
fn test_format_bytes_gigabytes() {
    assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    assert_eq!(format_bytes(2_684_354_560), "2.5 GB");
}

// ── format_number ───────────────────────────────────────────────────────────

#[test]
fn test_format_number_no_commas() {
    assert_eq!(format_number(0), "0");
    assert_eq!(format_number(1), "1");
    assert_eq!(format_number(999), "999");
}

#[test]
fn test_format_number_with_commas() {
    assert_eq!(format_number(1_000), "1,000");
    assert_eq!(format_number(12_345), "12,345");
    assert_eq!(format_number(243_302), "243,302");
    assert_eq!(format_number(1_000_000), "1,000,000");
    assert_eq!(format_number(1_234_567_890), "1,234,567,890");
}
