//! FTS5 query expressions built from raw user text.
//!
//! Shared by the storage backends: the libSQL backend binds these strings to
//! FTS5 `MATCH`, and the Grafeo backend evaluates the same expressions
//! against its own text index, so both match the same terms.

/// Converts a raw user query into a safe FTS5 MATCH expression.
///
/// Parameter binding stops SQL injection but does **not** stop FTS5 from
/// parsing the bound string as a query *expression* — `-`, `.`, `:`, `*`,
/// and friends are FTS5 syntax, so a term like `data-api` raises a syntax
/// error (#218). Each whitespace token is stripped of `"` and wrapped as a
/// quoted prefix term (`"token"*`), joined with `OR`. Returns `None` when
/// no tokenizable content remains.
pub(crate) fn to_fts_match_query(query: &str) -> Option<String> {
    let fts_query = query
        .split_whitespace()
        .filter_map(|w| {
            let sanitized: String = w.chars().filter(|c| *c != '"').collect();
            (!sanitized.is_empty()).then(|| format!("\"{sanitized}\"*"))
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    (!fts_query.is_empty()).then_some(fts_query)
}

/// Converts a multi-word query into an FTS5 *phrase* expression.
///
/// [`to_fts_match_query`] joins tokens with `OR`, which is right for a
/// natural-language query but loses an exact phrase entirely: every symbol
/// matching just the common word competes on equal footing, and BM25 then
/// ranks a unique hit inside a large field — a localization catalog stored as
/// one symbol's signature — below hundreds of short generic matches (#362).
/// Matching the phrase as a phrase gives the exact hit its own channel.
/// Returns `None` for single-word queries, where the two forms are identical.
pub(crate) fn to_fts_phrase_query(query: &str) -> Option<String> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| w.chars().filter(|c| *c != '"').collect::<String>())
        .filter(|w| !w.is_empty())
        .collect();
    (words.len() >= 2).then(|| format!("\"{}\"", words.join(" ")))
}
