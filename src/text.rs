//! Small text-handling utilities shared across crates.

/// Returns the longest prefix of `s` whose byte length does not exceed
/// `max_bytes`, snapping back to the nearest preceding UTF-8 char boundary
/// when the budget lands inside a multi-byte character.
///
/// This is the safe replacement for `&s[..max_bytes]` when `s` may contain
/// non-ASCII text and the caller has a byte budget rather than a char budget.
pub fn utf8_prefix_at_or_before(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    let mut end = max_bytes;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::utf8_prefix_at_or_before;

    #[test]
    fn returns_whole_string_when_under_budget() {
        assert_eq!(utf8_prefix_at_or_before("hello", 10), "hello");
    }

    #[test]
    fn returns_whole_string_when_at_budget() {
        assert_eq!(utf8_prefix_at_or_before("hello", 5), "hello");
    }

    #[test]
    fn truncates_ascii_at_budget() {
        assert_eq!(utf8_prefix_at_or_before("abcdef", 3), "abc");
    }

    #[test]
    fn walks_back_when_cut_lands_inside_multibyte_char() {
        // "é" is 2 bytes (0xC3 0xA9). With 20 'a's the total is 22 bytes;
        // a budget of 21 lands inside "é" and must walk back to 20.
        let s = format!("{}é", "a".repeat(20));
        assert_eq!(utf8_prefix_at_or_before(&s, 21), "a".repeat(20));
    }

    #[test]
    fn returns_empty_when_budget_lands_inside_leading_multibyte() {
        // 4-byte emoji at position 0; any budget < 4 (but > 0) walks back to 0.
        let s = "🦀tail";
        assert_eq!(utf8_prefix_at_or_before(s, 2), "");
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(utf8_prefix_at_or_before("", 10), "");
        assert_eq!(utf8_prefix_at_or_before("", 0), "");
    }

    #[test]
    fn handles_zero_budget() {
        assert_eq!(utf8_prefix_at_or_before("abc", 0), "");
    }
}
