// Rust guideline compliant 2025-10-17
//! Shared helpers for deriving the in-scope identifier of a Go import.
//!
//! Go imports are slash-separated paths and support *semantic import
//! versioning* (a path ending in `/vN`, e.g. `github.com/jackc/pgx/v5`). For
//! such a path the package identifier that code actually references is the
//! segment *before* the `/vN` (`pgx`), not the literal last segment (`v5`).
//!
//! Two subsystems must agree on this derivation:
//! - `unused_imports` (`mcp::tools::handlers::analysis`) — to find whether the
//!   import's identifier appears at a call site (#149 Bug 2).
//! - the reference resolver (`resolution::resolver`) — to map a selector
//!   qualifier (`foojobs`) back to the import path so same-named packages don't
//!   collide (#149 Bug 1).
//!
//! Keeping the logic here guarantees both stay consistent.

/// True if `seg` is a Go semantic-import-versioning segment: `v` followed by
/// one or more ASCII digits (`v2`, `v5`, `v11`). A segment that merely *starts*
/// with `v` (`revision`, `view`) is not a version segment.
fn is_version_segment(seg: &str) -> bool {
    seg.strip_prefix('v')
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// Returns the package identifier a bare (un-aliased) Go import path brings
/// into scope: the last path segment, except when that segment is a `/vN`
/// version marker, in which case the *preceding* segment is used.
///
/// - `net/url` -> `url`
/// - `github.com/golang-jwt/jwt/v5` -> `jwt`
/// - `github.com/jackc/pgx/v5` -> `pgx`
/// - `example.com/m/internal/foo/revision` -> `revision` (only `^v\d+$` triggers)
///
/// Returns `None` for an empty path.
pub fn package_identifier_from_path(path: &str) -> Option<&str> {
    let path = path.trim().trim_end_matches('/');
    if path.is_empty() {
        return None;
    }
    let mut segments = path.rsplit('/');
    let last = segments.next().unwrap_or(path);
    if is_version_segment(last) {
        // Prefer the segment before the version marker; if there is none
        // (a degenerate path that is just `v5`), fall back to the marker.
        return Some(segments.next().filter(|s| !s.is_empty()).unwrap_or(last));
    }
    Some(last)
}

/// Returns the identifier a Go import brings into scope, accounting for an
/// explicit alias encoded as `<path> as <alias>` (the convention used by the
/// Go extractor's Use nodes).
///
/// - `net/url` -> `url`
/// - `github.com/jackc/pgx/v5` -> `pgx`
/// - `github.com/jackc/pgx/v5 as pgxv5` -> `pgxv5`
///
/// Returns `None` when the derived identifier would be empty.
pub fn import_identifier(name: &str) -> Option<String> {
    let name = name.trim();
    // Aliased form: `<path> as <alias>` — the alias is what code references.
    if let Some((_, alias)) = name.rsplit_once(" as ") {
        let alias = alias.trim();
        return (!alias.is_empty()).then(|| alias.to_string());
    }
    package_identifier_from_path(name).map(ToString::to_string)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn plain_last_segment() {
        assert_eq!(import_identifier("net/url").as_deref(), Some("url"));
    }

    #[test]
    fn versioned_path_uses_preceding_segment() {
        assert_eq!(
            import_identifier("github.com/golang-jwt/jwt/v5").as_deref(),
            Some("jwt")
        );
        assert_eq!(
            import_identifier("github.com/jackc/pgx/v5").as_deref(),
            Some("pgx")
        );
    }

    #[test]
    fn explicit_alias_wins_over_version_logic() {
        assert_eq!(
            import_identifier("github.com/jackc/pgx/v5 as pgxv5").as_deref(),
            Some("pgxv5")
        );
    }

    #[test]
    fn non_version_trailing_segment_starting_with_v() {
        // Only `^v\d+$` triggers the version rule; `revision` is a normal name.
        assert_eq!(
            import_identifier("example.com/m/internal/foo/revision").as_deref(),
            Some("revision")
        );
    }

    #[test]
    fn version_detection_boundaries() {
        assert!(is_version_segment("v2"));
        assert!(is_version_segment("v11"));
        assert!(!is_version_segment("v"));
        assert!(!is_version_segment("revision"));
        assert!(!is_version_segment("v5beta"));
    }
}
