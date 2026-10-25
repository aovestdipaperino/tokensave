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

/// Split a compound name into individual words.
///
/// Handles camelCase, `PascalCase`, and `snake_case`:
/// - `getUserName` → `["get", "User", "Name"]`
/// - `process_request` → `["process", "request"]`
/// - `MAX_RETRIES` → `["MAX", "RETRIES"]`
pub fn split_compound(name: &str) -> Vec<&str> {
    if name.contains('_') {
        return name.split('_').filter(|s| !s.is_empty()).collect();
    }

    // camelCase / PascalCase splitting
    let bytes = name.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;

    for i in 1..bytes.len() {
        let cur = bytes[i] as char;
        let prev = bytes[i - 1] as char;

        // Split at lowercase→uppercase boundary (e.g. getUser → get|User)
        let boundary = prev.is_ascii_lowercase() && cur.is_ascii_uppercase();
        // Split at uppercase→uppercase+lowercase (e.g. XMLParser → XML|Parser)
        let acronym_end = i + 1 < bytes.len()
            && prev.is_ascii_uppercase()
            && cur.is_ascii_uppercase()
            && (bytes[i + 1] as char).is_ascii_lowercase();

        if boundary || acronym_end {
            if i > start {
                parts.push(&name[start..i]);
            }
            start = i;
        }
    }
    if start < name.len() {
        parts.push(&name[start..]);
    }
    parts
}

/// Returns `true` if `word` looks like CamelCase.
///
/// The word must contain at least one uppercase letter after the first character
/// and consist only of ASCII alphanumeric characters.
pub fn is_camel_case(word: &str) -> bool {
    if word.len() < 2 {
        return false;
    }
    // Must be all alphanumeric
    if !word.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    // Must have at least one uppercase letter after the first char
    word[1..].chars().any(|c| c.is_ascii_uppercase())
}

/// Space-joined camelCase word segments for the FTS `search_terms` column.
///
/// The unicode61 tokenizer already splits on every non-alphanumeric byte
/// (`_`, `::`, `/`, `.`), so `snake_case` names and path segments are indexed
/// as separate tokens on their own. What it cannot derive are the words
/// inside a camelCase identifier — `updateCloudClient` stays one token.
/// Emit only those inner words, deduplicated case-insensitively, so the
/// column adds signal without repeating what the tokenizer already produces.
#[must_use]
pub fn search_terms(name: &str, qualified_name: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    for word in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .chain(qualified_name.split(|c: char| !c.is_ascii_alphanumeric()))
    {
        if !is_camel_case(word) {
            continue;
        }
        for part in split_compound(word) {
            if part.len() >= 2 && seen.insert(part.to_ascii_lowercase()) {
                out.push(part);
            }
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::{is_camel_case, search_terms, split_compound, utf8_prefix_at_or_before};

    #[test]
    fn split_compound_handles_camel_snake_screaming() {
        assert_eq!(split_compound("getUserName"), vec!["get", "User", "Name"]);
        assert_eq!(
            split_compound("process_request"),
            vec!["process", "request"]
        );
        assert_eq!(split_compound("MAX_RETRIES"), vec!["MAX", "RETRIES"]);
        assert_eq!(split_compound("XMLParser"), vec!["XML", "Parser"]);
    }

    #[test]
    fn is_camel_case_classifies() {
        assert!(is_camel_case("UserService"));
        assert!(is_camel_case("processRequest"));
        assert!(!is_camel_case("user"));
        assert!(!is_camel_case("U"));
        assert!(!is_camel_case("process_request"));
    }

    #[test]
    fn search_terms_emits_camel_parts_only() {
        // Snake_case name: unicode61 already splits it — nothing to add.
        assert_eq!(
            search_terms(
                "rerank_candidates",
                "src/context/ranking.rs::rerank_candidates"
            ),
            ""
        );
        // CamelCase name and path segment produce inner words.
        let terms = search_terms(
            "updateCloudClient",
            "src/api/CloudSync.ts::updateCloudClient",
        );
        assert_eq!(terms, "update Cloud Client Sync");
    }

    #[test]
    fn search_terms_dedupes_case_insensitively() {
        let terms = search_terms("ParseJson", "parser/ParseJson.kt::ParseJson");
        assert_eq!(terms, "Parse Json");
    }

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
