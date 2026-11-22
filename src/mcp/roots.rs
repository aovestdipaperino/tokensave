//! Choosing a project from the MCP client's roots.
//!
//! A server started outside every indexed project has no default project
//! (#606). When the client declared the `roots` capability, a call without
//! `graph_root` asks the client for its roots (`roots/list`) while the call
//! is running and answers from the indexed project that contains them.
//!
//! The roots are requested on every such call, never once per session and
//! never cached: behind an MCP proxy one server process and one session serve
//! many clients, the proxy forwards `roots/list` only to the client whose call
//! is in flight (answering an empty list otherwise), and it does not forward
//! `notifications/roots/list_changed`. Only a per-call answer is known to
//! belong to the caller.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

/// How long a call waits for the client to answer `roots/list`. A client
/// answers it without user interaction, so this only bounds a client that
/// never answers; the call is then refused as it was before roots were asked.
const ROOTS_TIMEOUT: Duration = Duration::from_secs(5);

/// Requests this server sends to its client, and the answers it waits for.
#[derive(Default)]
pub(crate) struct ClientRequests {
    /// Where request lines go; set while the serve loop runs.
    outbound: Mutex<Option<mpsc::UnboundedSender<String>>>,
    /// The calls waiting for an answer, by request id.
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
}

impl ClientRequests {
    /// Starts sending requests through `outbound`.
    pub(crate) fn connect(&self, outbound: mpsc::UnboundedSender<String>) {
        if let Ok(mut slot) = self.outbound.lock() {
            *slot = Some(outbound);
        }
    }

    /// Stops sending requests; calls still waiting get no answer.
    pub(crate) fn disconnect(&self) {
        if let Ok(mut slot) = self.outbound.lock() {
            *slot = None;
        }
        if let Ok(mut pending) = self.pending.lock() {
            pending.clear();
        }
    }

    /// Hands `line` to the call waiting for it when it answers a pending
    /// request. `false` means it is not such an answer and is handled as an
    /// incoming request.
    pub(crate) fn deliver(&self, line: &str) -> bool {
        // A session that never asked has nothing pending, so its requests are
        // not parsed twice.
        if self
            .pending
            .lock()
            .map_or(true, |pending| pending.is_empty())
        {
            return false;
        }
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        if message.get("method").is_some() {
            return false;
        }
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return false;
        };
        let Some(waiter) = self
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id))
        else {
            return false;
        };
        let _ = waiter.send(message);
        true
    }

    /// Sends `method` to the client and returns the `result` of its answer.
    /// `None` when the serve loop is not running, or the client answered with
    /// an error or not in time.
    async fn request(&self, method: &str) -> Option<Value> {
        let outbound = self.outbound.lock().ok()?.clone()?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (waiter, answer) = oneshot::channel();
        self.pending.lock().ok()?.insert(id, waiter);
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        let answer = if outbound.send(format!("{line}\n")).is_ok() {
            tokio::time::timeout(ROOTS_TIMEOUT, answer)
                .await
                .ok()
                .and_then(Result::ok)
        } else {
            None
        };
        // Answered, timed out or unsent: the entry must not outlive the call.
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&id);
        }
        answer?.get("result").cloned()
    }

    /// The URIs of the client's roots, or `None` when it could not be asked.
    pub(crate) async fn roots(&self) -> Option<Vec<String>> {
        let result = self.request("roots/list").await?;
        let roots = result.get("roots")?.as_array()?;
        Some(
            roots
                .iter()
                .filter_map(|root| root.get("uri")?.as_str().map(str::to_owned))
                .collect(),
        )
    }
}

/// Whether `line` is a JSON-RPC response rather than a request: an answer to
/// a request of this server that came too late to be delivered. It gets no
/// reply.
pub(crate) fn is_response(line: &str) -> bool {
    serde_json::from_str::<Value>(line).is_ok_and(|message| {
        message.get("method").is_none()
            && (message.get("result").is_some() || message.get("error").is_some())
    })
}

/// The distinct indexed projects that contain the client's roots, in the
/// client's order. A root that is not a local absolute path, or lies in no
/// indexed project, is skipped.
pub(crate) fn projects_for_roots(uris: &[String]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut projects = Vec::new();
    for uri in uris {
        let Some(path) = root_uri_to_path(uri).filter(|path| path.is_absolute()) else {
            continue;
        };
        let Some(project) = crate::config::discover_project_root(&path) else {
            continue;
        };
        // Two roots inside one project select it once.
        if seen.insert(project.canonicalize().unwrap_or_else(|_| project.clone())) {
            projects.push(project);
        }
    }
    projects
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
pub fn root_uri_to_path(uri: &str) -> Option<PathBuf> {
    root_uri_to_path_on(uri, cfg!(windows))
}

/// The platform-independent half, so both behaviours stay testable wherever the
/// suite runs — CI is Linux, and gating the drive-letter rule on `cfg!(windows)`
/// alone would leave the Windows case, the one this exists for, uncovered.
///
/// `strip_drive_letter` belongs to the host, not the URI: `/C:/x` is a perfectly
/// legal POSIX path, so stripping it there would corrupt a real directory name.
fn root_uri_to_path_on(uri: &str, strip_drive_letter: bool) -> Option<PathBuf> {
    if uri.is_empty() {
        return None;
    }
    let path = match uri.strip_prefix("file://") {
        // `file://localhost/x` is the spelled-out form of `file:///x`, and
        // `file://127.0.0.1/x` / `file://[::1]/x` name the same local host.
        // The guard proved `rest`'s first `/`-delimited segment is one of
        // those three authorities, all pure ASCII, so slicing off exactly
        // that segment's byte length — not a hardcoded constant — cannot
        // land inside a multi-byte character and cannot go out of bounds.
        Some(rest) if authority_is_localhost(rest) => {
            let authority_len = rest.split('/').next().unwrap_or_default().len();
            percent_decode(&rest[authority_len..])?
        }
        // `file://host/share` uses a non-local authority. UNC roots are outside
        // this fallback's current scope, so do not reinterpret one as a relative path.
        Some(rest) if !rest.starts_with('/') => return None,
        Some(rest) => percent_decode(rest)?,
        None if uri.contains("://") => return None,
        None => uri.to_string(),
    };
    // `file://localhost` with no path decodes to "" — nothing to serve.
    if path.is_empty() {
        return None;
    }

    // `file:///D:/Dev/app` decodes to `/D:/Dev/app`; drop the leading slash that
    // belongs to the URI, not to the path, when a drive letter follows it.
    let path = match path.strip_prefix('/') {
        Some(rest) if strip_drive_letter && has_drive_prefix(rest) => rest.to_string(),
        _ => path,
    };

    Some(PathBuf::from(path))
}

/// True when the URI's authority component refers to the local machine.
///
/// Accepts `localhost`, `127.0.0.1`, and `[::1]` (host names are
/// case-insensitive).
///
/// Splits on `/` rather than slicing at a byte offset: this input comes
/// straight from an MCP client, and a byte-indexed slice panics when a
/// multi-byte character straddles the cut (`localhosé/…` puts byte 9 inside
/// the `é`) — a malformed root must never take the server down.
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

/// Decodes `%XX` escapes, leaving malformed sequences untouched. Returns
/// `None` when the decoded bytes are not valid UTF-8.
///
/// Decoding is done on bytes rather than chars so that a percent-encoded
/// multi-byte UTF-8 character reassembles correctly. An invalid decode must
/// reject the root rather than fall back to lossy replacement: `%C3`, `%FF`,
/// and every truncated multi-byte sequence would all collapse into the same
/// U+FFFD path — a directory the client never named, which a registered
/// project could then wrongly match.
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    if !bytes.contains(&b'%') {
        return Some(input.to_string());
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
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::{has_drive_prefix, percent_decode, root_uri_to_path, root_uri_to_path_on};
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
    fn ip_literal_authorities_are_the_local_machine() {
        // `127.0.0.1` and `[::1]` are as long, respectively longer/shorter
        // than `localhost` — the authority-slicing must key off the actual
        // matched authority, not a hardcoded length, or these either
        // misparse or panic on a short path.
        assert_eq!(unix("file://127.0.0.1/home/user/app"), "/home/user/app");
        assert_eq!(win("file://127.0.0.1/D:/Dev/app"), "D:/Dev/app");
        assert_eq!(unix("file://[::1]/home/user/app"), "/home/user/app");
        assert_eq!(win("file://[::1]/D:/Dev/app"), "D:/Dev/app");
        // Regression: `[::1]` is shorter than `localhost`, so a URI whose
        // path is short enough previously indexed out of bounds and panicked.
        assert_eq!(unix("file://[::1]/x"), "/x");
        // A host that merely resembles a local authority is still a real host.
        assert!(root_uri_to_path("file://127.0.0.1.evil.com/x").is_none());
    }

    #[test]
    fn a_multibyte_authority_is_rejected_without_panicking() {
        // This input arrives from an MCP client. A byte-indexed authority
        // check panicked here: `localhosé` puts byte 9 inside the two-byte
        // `é`, and slicing off a char boundary aborts — letting a malformed
        // root take the whole server down.
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
        assert_eq!(percent_decode("100%"), Some("100%".to_string()));
        assert_eq!(percent_decode("a%zz"), Some("a%zz".to_string()));
        assert_eq!(percent_decode("a%4"), Some("a%4".to_string()));
        assert_eq!(percent_decode("%41"), Some("A".to_string()));
    }

    #[test]
    fn escapes_decoding_to_invalid_utf8_reject_the_root() {
        // `%C3` is a lone UTF-8 lead byte, `%FF` a byte no UTF-8 sequence
        // allows, `%E2%82` a truncated three-byte sequence. Lossy replacement
        // would collapse all three into the same `…/�` path — a directory the
        // client never named — so the root is rejected instead.
        assert!(root_uri_to_path("file:///tmp/%C3").is_none());
        assert!(root_uri_to_path("file:///tmp/%FF").is_none());
        assert!(root_uri_to_path("file:///tmp/%E2%82").is_none());
        assert!(root_uri_to_path("file://localhost/%FF").is_none());
        // A complete multi-byte sequence still decodes.
        assert_eq!(percent_decode("caf%C3%A9"), Some("café".to_string()));
    }

    #[test]
    fn a_localhost_authority_without_a_path_is_rejected() {
        // `file://localhost` leaves an empty path once the authority is
        // sliced off — nothing to serve.
        assert!(root_uri_to_path("file://localhost").is_none());
        assert!(root_uri_to_path("file://[::1]").is_none());
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
    fn only_responses_are_responses() {
        assert!(super::is_response(
            r#"{"jsonrpc":"2.0","id":3,"result":{}}"#
        ));
        assert!(super::is_response(
            r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"x"}}"#
        ));
        assert!(!super::is_response(
            r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#
        ));
        assert!(!super::is_response(r#"{"jsonrpc":"2.0","id":3}"#));
        assert!(!super::is_response("not json"));
    }
}
