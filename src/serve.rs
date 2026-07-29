use std::path::Path;
use tokensave::tokensave::TokenSave;

/// Opens an existing project, or tells the user to run `tokensave init` first.
pub async fn ensure_initialized(project_path: &Path) -> tokensave::errors::Result<TokenSave> {
    if TokenSave::is_initialized(project_path) {
        return TokenSave::open(project_path).await;
    }
    Err(tokensave::errors::TokenSaveError::Config {
        message: format!(
            "no TokenSave index found at '{}' — run 'tokensave init' first",
            project_path.display()
        ),
    })
}

/// Fallback for `serve`: when CWD-based discovery fails, check the global DB
/// for registered projects. When multiple projects exist, pick the best match
/// against cwd: prefer a project that is an ancestor of cwd (cwd is inside the
/// project), then a project that is a descendant of cwd (project is under cwd).
/// Among multiple matches, the deepest (most specific) path wins.
pub async fn resolve_serve_from_global_db() -> Option<std::path::PathBuf> {
    let gdb = tokensave::global_db::GlobalDb::open().await?;
    let mut paths: Vec<String> = gdb.list_project_paths().await;
    // Keep only projects whose .tokensave dir still exists on disk.
    paths.retain(|p| {
        std::path::Path::new(p)
            .join(".tokensave/tokensave.db")
            .exists()
    });
    if paths.len() == 1 {
        return Some(std::path::PathBuf::from(paths.remove(0)));
    }
    if paths.is_empty() {
        return None;
    }

    // Multiple projects — try to resolve using cwd.
    let cwd = std::env::current_dir().ok()?;
    let cwd = cwd.canonicalize().unwrap_or(cwd);

    // Priority 1: cwd is inside a project (project is ancestor of cwd).
    // Pick the deepest ancestor (most specific match).
    let mut ancestors: Vec<_> = paths
        .iter()
        .filter_map(|p| {
            let pp = std::path::Path::new(p).canonicalize().ok()?;
            cwd.starts_with(&pp)
                .then(|| (pp.components().count(), p.clone()))
        })
        .collect();
    ancestors.sort_by_key(|a| std::cmp::Reverse(a.0)); // deepest first
    if let Some((_, best)) = ancestors.into_iter().next() {
        return Some(std::path::PathBuf::from(best));
    }

    // Priority 2: a project is under cwd (cwd is ancestor of project).
    // Pick the shallowest descendant (closest child).
    let mut descendants: Vec<_> = paths
        .iter()
        .filter_map(|p| {
            let pp = std::path::Path::new(p).canonicalize().ok()?;
            pp.starts_with(&cwd)
                .then(|| (pp.components().count(), p.clone()))
        })
        .collect();
    descendants.sort_by_key(|a| a.0); // shallowest first
    if let Some((_, best)) = descendants.into_iter().next() {
        return Some(std::path::PathBuf::from(best));
    }

    // No cwd-based match — report the ambiguity.
    eprintln!("Multiple tokensave projects found — pass -p <path> to select one:");
    for p in &paths {
        eprintln!("  {p}");
    }
    None
}

/// Last-resort fallback for `serve`: peek at the first stdin line to read the
/// MCP `initialize` request's `roots` array.  If a root matches a registered
/// project, return its path.  The raw line is stored in `out` so the caller
/// can replay it into the MCP transport (the server still needs to see it).
pub async fn resolve_serve_from_mcp_roots(out: &mut Option<String>) -> Option<std::path::PathBuf> {
    use tokio::io::AsyncBufReadExt;
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line = String::new();
    // Read the first non-empty line (should be the `initialize` request).
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => return None, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    *out = Some(line.trim().to_string());

    let parsed: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let roots = parsed.pointer("/params/roots").and_then(|v| v.as_array())?;

    let gdb = tokensave::global_db::GlobalDb::open().await?;
    let registered: Vec<RegisteredProject> = gdb
        .list_project_paths()
        .await
        .into_iter()
        .filter_map(|p| {
            let pb = std::path::PathBuf::from(p);
            if pb.join(".tokensave/tokensave.db").exists() {
                let canonical = pb.canonicalize().ok();
                Some(RegisteredProject {
                    path: pb,
                    canonical,
                })
            } else {
                None
            }
        })
        .collect();

    // Try each root URI — first match wins.
    for root in roots {
        let uri = root.get("uri").and_then(|v| v.as_str()).unwrap_or_default();
        let Some(root_path) = root_uri_to_path(uri) else {
            tracing::debug!("skipping unsupported root URI: {}", uri);
            continue;
        };
        // Exact match: the root IS a registered project.
        if let Some(hit) = find_registered_project(&registered, &root_path) {
            tracing::info!("serving from MCP root: {}", hit.display());
            return Some(hit);
        }
        // Walk up from the root to find the nearest enclosing project.
        if let Some(discovered) = tokensave::config::discover_project_root(&root_path) {
            tracing::info!(
                "serving from discovered project under MCP root: {}",
                discovered.display()
            );
            return Some(discovered);
        }
    }
    None
}

/// A project registered in the global database.
struct RegisteredProject {
    path: std::path::PathBuf,
    canonical: Option<std::path::PathBuf>,
}

/// Finds the registered project that names the same directory as `root_path`.
///
/// Compares canonically as well as literally so that `D:\Dev\app` and
/// `D:/Dev/app` — the same directory spelled two ways — are recognized as one.
/// Canonicalization failure (e.g. the path does not exist) only disables the
/// canonical comparison; the literal one still applies. Returns the registered
/// spelling, not the root URI's, so downstream path comparisons keep working
/// against what the global DB recorded.
fn find_registered_project(
    registered: &[RegisteredProject],
    root_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let canonical_root = root_path.canonicalize().ok();
    registered
        .iter()
        .find(|rp| {
            rp.path == root_path
                || match (rp.canonical.as_ref(), canonical_root.as_ref()) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                }
        })
        .map(|rp| rp.path.clone())
}

/// Converts an MCP `roots` entry into a filesystem path.
///
/// Stripping a bare `file://` prefix is not enough. A client spells a Windows
/// root `file:///D:/Dev/app`, which leaves `/D:/Dev/app` — a path that matches
/// no registered project and that `discover_project_root` cannot walk, so the
/// whole roots-based fallback was dead on Windows. Percent-encoding was ignored
/// too, so any path containing a space (`file:///C:/My%20Project`) failed
/// everywhere. This is the inverse of `agents::kiro::file_resource_uri`, which
/// already produces exactly this form.
///
/// Values that are not URIs at all are returned as-is, since some clients send
/// a plain path.
fn root_uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    root_uri_to_path_on(uri, cfg!(windows))
}

/// The platform-independent half, so both behaviours stay testable wherever the
/// suite runs — CI is Linux, and gating the drive-letter rule on `cfg!(windows)`
/// alone would leave the Windows case, the one this exists for, uncovered.
///
/// `strip_drive_letter` belongs to the host, not the URI: `/C:/x` is a perfectly
/// legal POSIX path, so stripping it there would corrupt a real directory name.
fn root_uri_to_path_on(uri: &str, strip_drive_letter: bool) -> Option<std::path::PathBuf> {
    if uri.is_empty() {
        return None;
    }
    let path = match uri.strip_prefix("file://") {
        // `file://localhost/x` is the spelled-out form of `file:///x`. Host
        // names are case-insensitive, so `LOCALHOST` counts too. The guard
        // proved the authority is exactly "localhost" — nine ASCII bytes — so
        // this byte-indexed slice cannot land inside a multi-byte character.
        Some(rest) if authority_is_localhost(rest) => percent_decode(&rest["localhost".len()..]),
        // `file://host/share` uses a non-local authority. UNC roots are outside
        // this fallback's current scope, so do not reinterpret one as a relative path.
        Some(rest) if !rest.starts_with('/') => return None,
        Some(rest) => percent_decode(rest),
        None if uri.contains("://") => return None,
        None => uri.to_string(),
    };

    // `file:///D:/Dev/app` decodes to `/D:/Dev/app`; drop the leading slash that
    // belongs to the URI, not to the path, when a drive letter follows it.
    let path = match path.strip_prefix('/') {
        Some(rest) if strip_drive_letter && has_drive_prefix(rest) => rest.to_string(),
        _ => path,
    };

    Some(std::path::PathBuf::from(path))
}

/// True when the URI's authority component refers to the local machine.
///
/// Accepts `localhost`, `127.0.0.1`, and `[::1]` (host names are
/// case-insensitive).
///
/// Splits on `/` rather than slicing at a byte offset: this input comes
/// straight from an MCP client's `initialize` request, and a byte-indexed slice
/// panics when a multi-byte character straddles the cut (`localhosé/…` puts
/// byte 9 inside the `é`) — a malformed root must never take the server down.
fn authority_is_localhost(rest: &str) -> bool {
    rest.split('/').next().is_some_and(|authority| {
        authority.eq_ignore_ascii_case("localhost")
            || authority == "127.0.0.1"
            || authority == "[::1]"
    })
}

/// True when `path` starts with a Windows drive designator such as `C:`.
fn has_drive_prefix(path: &str) -> bool {
    let mut chars = path.chars();
    matches!((chars.next(), chars.next()), (Some(c), Some(':')) if c.is_ascii_alphabetic())
}

/// Decodes `%XX` escapes, leaving malformed sequences untouched.
///
/// Decoding is done on bytes rather than chars so that a percent-encoded
/// multi-byte UTF-8 character reassembles correctly.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    if !bytes.contains(&b'%') {
        return input.to_string();
    }
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        find_registered_project, has_drive_prefix, percent_decode, root_uri_to_path,
        root_uri_to_path_on,
    };
    use std::path::PathBuf;

    /// Resolves as a Windows host would.
    fn win(uri: &str) -> String {
        render(root_uri_to_path_on(uri, true))
    }

    /// Resolves as a Unix host would.
    fn unix(uri: &str) -> String {
        render(root_uri_to_path_on(uri, false))
    }

    fn render(path: Option<PathBuf>) -> String {
        path.unwrap_or_else(|| PathBuf::from("<none>"))
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn windows_root_drops_the_uri_leading_slash() {
        assert_eq!(win("file:///D:/Dev/app"), "D:/Dev/app");
        assert_eq!(win("file:///c:/dev/app"), "c:/dev/app");
    }

    #[test]
    fn a_drive_letter_is_only_a_drive_letter_on_windows() {
        // `/C:/x` is a legal POSIX path; mangling it into a relative `C:/x`
        // would point at a different directory entirely.
        assert_eq!(unix("file:///C:/x"), "/C:/x");
    }

    #[test]
    fn windows_keeps_the_leading_slash_without_a_drive_prefix() {
        assert_eq!(win("file:///home/user/app"), "/home/user/app");
        // A single-letter first component is not a drive letter.
        assert_eq!(win("file:///d/dev/app"), "/d/dev/app");
    }

    #[test]
    fn localhost_authority_is_the_local_machine() {
        // `file://localhost/x` is the spelled-out form of `file:///x`, unlike a
        // real host, which would be a UNC share. Host names are
        // case-insensitive, so any casing counts.
        assert_eq!(unix("file://localhost/home/user/app"), "/home/user/app");
        assert_eq!(win("file://localhost/D:/Dev/app"), "D:/Dev/app");
        assert_eq!(unix("file://LOCALHOST/home/user/app"), "/home/user/app");
        assert_eq!(unix("file://LocalHost/home/user/app"), "/home/user/app");
        // A host that merely *starts* with the word is a real (UNC) host.
        assert!(root_uri_to_path("file://localhostname/share").is_none());
    }

    #[test]
    fn a_multibyte_authority_is_rejected_without_panicking() {
        // This input arrives from an MCP client's `initialize` request. A
        // byte-indexed authority check panicked here: `localhosé` puts byte 9
        // inside the two-byte `é`, and slicing off a char boundary aborts —
        // letting a malformed root take the whole server down.
        assert!(root_uri_to_path("file://localhosé/share").is_none());
        assert!(root_uri_to_path("file://locälhost/x").is_none());
        assert!(root_uri_to_path("file://é/x").is_none());
    }

    #[test]
    fn percent_escapes_are_decoded() {
        assert_eq!(win("file:///C:/My%20Project"), "C:/My Project");
        assert_eq!(win("file:///home/user/my%20app"), "/home/user/my app");
        assert_eq!(win("file:///home/user/caf%C3%A9"), "/home/user/café");
    }

    #[test]
    fn malformed_escapes_are_left_alone() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%zz"), "a%zz");
        assert_eq!(percent_decode("a%4"), "a%4");
        assert_eq!(percent_decode("%41"), "A");
    }

    #[test]
    fn bare_paths_pass_through() {
        assert_eq!(win("/home/user/app"), "/home/user/app");
        assert_eq!(win("D:/Dev/app"), "D:/Dev/app");
        // The host-dependent entry point resolves for the platform it targets.
        assert!(root_uri_to_path("/home/user/app").is_some());
    }

    #[test]
    fn unsupported_uris_are_skipped() {
        assert!(root_uri_to_path("").is_none());
        assert!(root_uri_to_path("https://example.com/app").is_none());
        // UNC authorities are intentionally outside this fallback's current scope.
        assert!(root_uri_to_path("file://server/share").is_none());
    }

    #[test]
    fn drive_prefix_detection() {
        assert!(has_drive_prefix("C:/x"));
        assert!(has_drive_prefix("z:"));
        assert!(!has_drive_prefix("/C:/x"));
        assert!(!has_drive_prefix("home/user"));
        assert!(!has_drive_prefix("1:/x"));
        assert!(!has_drive_prefix(""));
    }

    #[test]
    fn registered_match_literal_even_when_the_path_does_not_exist() {
        // Canonicalization fails on a nonexistent path; the literal comparison
        // must still hold on its own.
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("missing");
        assert!(!missing.exists());

        let registered = vec![missing.to_string_lossy().into_owned()];
        assert_eq!(
            find_registered_project(&prep(registered), &missing),
            Some(missing)
        );
    }

    #[test]
    fn registered_match_via_canonicalization_returns_the_registered_spelling() {
        // `project/../project` names the same existing directory as `project`
        // without stepping outside the temp dir; only canonicalization can
        // equate the two spellings.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let registered = vec![project.to_string_lossy().into_owned()];
        let alias = project.join("..").join("project");
        assert_ne!(alias.as_path(), project.as_path());
        assert_eq!(
            find_registered_project(&prep(registered), &alias),
            Some(project.clone())
        );
    }

    fn prep(registered: Vec<String>) -> Vec<super::RegisteredProject> {
        registered
            .into_iter()
            .map(|p| {
                let pb = PathBuf::from(p);
                let canonical = pb.canonicalize().ok();
                super::RegisteredProject {
                    path: pb,
                    canonical,
                }
            })
            .collect()
    }

    #[test]
    fn registered_mismatch_between_two_existing_directories() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let registered = vec![a.to_string_lossy().into_owned()];
        assert_eq!(find_registered_project(&prep(registered), &b), None);
    }
}
