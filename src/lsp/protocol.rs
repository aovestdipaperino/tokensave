// Rust guideline compliant 2025-10-17
//! LSP protocol message types.
//!
//! Only the subset used by tokensave's resolution pass: `initialize`,
//! `initialized`, `textDocument/didOpen`, `textDocument/definition`, and
//! `shutdown` / `exit`. Each message is mapped to a serde-friendly struct;
//! fields are flattened to JSON via `serde_json::Value` where the LSP spec
//! allows multiple shapes (e.g. `definition` may return `Location` or
//! `LocationLink`).
//!
//! See <https://microsoft.github.io/language-server-protocol/> for the full
//! specification. The types here pass `#[serde(skip_serializing_if = "...")]`
//! liberally so missing fields stay missing on the wire (servers reject
//! `null` in places the spec marks optional).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A `file:///...` URI as required by the LSP spec for `textDocument` methods.
pub type DocumentUri = String;

/// Build a `file://` URI from an absolute filesystem path. Percent-encodes
/// the path components so spaces and Unicode survive the round trip.
pub fn path_to_uri(path: &std::path::Path) -> DocumentUri {
    // We encode with a small explicit table so that we don't pull in a URL
    // crate just for this. RFC 3986 reserves a small set we must escape.
    let mut buf = String::from("file://");
    for byte in path.to_string_lossy().as_bytes() {
        let c = *byte;
        let needs_escape =
            !(c.is_ascii_alphanumeric() || matches!(c, b'/' | b'-' | b'_' | b'.' | b'~' | b':'));
        if needs_escape {
            use std::fmt::Write as _;
            let _ = write!(&mut buf, "%{c:02X}");
        } else {
            buf.push(c as char);
        }
    }
    buf
}

/// `initialize` request parameters. Only the fields tokensave actually sets
/// are modeled; the spec allows many more.
#[derive(Debug, Clone, Serialize)]
pub struct InitializeParams {
    /// PID of the parent process. Servers terminate themselves if it dies.
    #[serde(rename = "processId")]
    pub process_id: Option<u32>,

    /// Root URI of the workspace (file://...).
    #[serde(rename = "rootUri")]
    pub root_uri: DocumentUri,

    /// Optional server-specific options forwarded by the adapter.
    #[serde(
        rename = "initializationOptions",
        skip_serializing_if = "Option::is_none"
    )]
    pub initialization_options: Option<Value>,

    /// Capabilities advertised by tokensave (the client). Most servers only
    /// require `textDocument` to be present.
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ClientCapabilities {
    #[serde(rename = "textDocument", skip_serializing_if = "Option::is_none")]
    pub text_document: Option<TextDocumentClientCapabilities>,

    #[serde(rename = "workspace", skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceClientCapabilities>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TextDocumentClientCapabilities {
    /// We don't need rich definition responses; the server may still send
    /// `LocationLink` even with `linkSupport=false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<DefinitionClientCapabilities>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DefinitionClientCapabilities {
    #[serde(rename = "linkSupport", skip_serializing_if = "Option::is_none")]
    pub link_support: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkspaceClientCapabilities {
    /// Many servers only run if the client claims workspace folder support.
    #[serde(rename = "workspaceFolders", skip_serializing_if = "Option::is_none")]
    pub workspace_folders: Option<bool>,
}

/// `initialize` response. Most fields are unused by tokensave; we deserialise
/// only what we need to detect `definitionProvider`.
#[derive(Debug, Clone, Deserialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCapabilities {
    /// Either a bool or an object per the spec; we accept either.
    #[serde(rename = "definitionProvider", default)]
    pub definition_provider: Option<Value>,
}

impl ServerCapabilities {
    /// True when the server advertises `textDocument/definition` support.
    pub fn supports_definition(&self) -> bool {
        match &self.definition_provider {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => true, // object form means yes-with-options
        }
    }
}

/// `textDocument/didOpen` notification parameters.
#[derive(Debug, Clone, Serialize)]
pub struct DidOpenTextDocumentParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentItem,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextDocumentItem {
    pub uri: DocumentUri,
    /// Per the LSP spec, e.g. "rust", "go", "c", "cpp", "lua".
    #[serde(rename = "languageId")]
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

/// `textDocument/definition` request parameters.
#[derive(Debug, Clone, Serialize)]
pub struct DefinitionParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextDocumentIdentifier {
    pub uri: DocumentUri,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    /// 0-based line number per the LSP spec.
    pub line: u32,
    /// 0-based UTF-16 character offset per the LSP spec.
    pub character: u32,
}

/// `Range` in a document. Both endpoints are inclusive on the line dimension
/// and exclusive on the character dimension.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// `Location` — a `(uri, range)` pointer. The `definition` response may be
/// a single `Location`, an array of `Location`, or an array of `LocationLink`.
#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub uri: DocumentUri,
    pub range: Range,
}

/// `LocationLink` — richer variant returned when the client claims
/// `linkSupport`. Tokensave only consumes `targetUri` + `targetRange`.
#[derive(Debug, Clone, Deserialize)]
pub struct LocationLink {
    #[serde(rename = "targetUri")]
    pub target_uri: DocumentUri,
    #[serde(rename = "targetRange")]
    pub target_range: Range,
}

/// Normalised result of a `textDocument/definition` request. Servers may
/// return `null`, a `Location`, an array of `Location`, or an array of
/// `LocationLink` — `parse_definition_response` collapses all four into this
/// flat list.
#[derive(Debug, Clone)]
pub struct DefinitionTarget {
    pub uri: DocumentUri,
    pub range: Range,
}

/// Parses a raw `definition` response value into the unified target list.
/// Returns an empty Vec for the `null` case.
pub fn parse_definition_response(value: &Value) -> Vec<DefinitionTarget> {
    use serde_json::Value as V;
    match value {
        V::Null => Vec::new(),
        V::Object(_) => {
            // Single Location.
            serde_json::from_value::<Location>(value.clone())
                .ok()
                .map(|l| {
                    vec![DefinitionTarget {
                        uri: l.uri,
                        range: l.range,
                    }]
                })
                .unwrap_or_default()
        }
        V::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Ok(l) = serde_json::from_value::<Location>(item.clone()) {
                    out.push(DefinitionTarget {
                        uri: l.uri,
                        range: l.range,
                    });
                } else if let Ok(l) = serde_json::from_value::<LocationLink>(item.clone()) {
                    out.push(DefinitionTarget {
                        uri: l.target_uri,
                        range: l.target_range,
                    });
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn path_to_uri_basic() {
        let p = std::path::PathBuf::from("/tmp/foo bar.rs");
        let uri = path_to_uri(&p);
        assert!(uri.starts_with("file:///tmp/foo"));
        assert!(uri.contains("%20"), "spaces should be percent-encoded");
        assert!(uri.ends_with("bar.rs"));
    }

    #[test]
    fn path_to_uri_no_escape_for_safe_chars() {
        let p = std::path::PathBuf::from("/usr/local/lib/main.rs");
        assert_eq!(path_to_uri(&p), "file:///usr/local/lib/main.rs");
    }

    #[test]
    fn definition_response_null_yields_empty() {
        assert!(parse_definition_response(&Value::Null).is_empty());
    }

    #[test]
    fn definition_response_single_location() {
        let v = json!({
            "uri": "file:///foo/bar.rs",
            "range": {
                "start": {"line": 1, "character": 2},
                "end":   {"line": 1, "character": 6}
            }
        });
        let targets = parse_definition_response(&v);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].uri, "file:///foo/bar.rs");
        assert_eq!(targets[0].range.start.line, 1);
    }

    #[test]
    fn definition_response_array_of_locations() {
        let v = json!([
            {
                "uri": "file:///a.rs",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end":   {"line": 0, "character": 1}
                }
            },
            {
                "uri": "file:///b.rs",
                "range": {
                    "start": {"line": 5, "character": 0},
                    "end":   {"line": 5, "character": 4}
                }
            }
        ]);
        let targets = parse_definition_response(&v);
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[1].uri, "file:///b.rs");
        assert_eq!(targets[1].range.start.line, 5);
    }

    #[test]
    fn definition_response_array_of_location_links() {
        let v = json!([{
            "targetUri": "file:///x.rs",
            "targetRange": {
                "start": {"line": 10, "character": 0},
                "end":   {"line": 10, "character": 5}
            },
            "targetSelectionRange": {
                "start": {"line": 10, "character": 4},
                "end":   {"line": 10, "character": 5}
            }
        }]);
        let targets = parse_definition_response(&v);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].uri, "file:///x.rs");
        assert_eq!(targets[0].range.start.line, 10);
    }

    #[test]
    fn server_capabilities_definition_provider_bool() {
        let caps = ServerCapabilities {
            definition_provider: Some(Value::Bool(true)),
        };
        assert!(caps.supports_definition());

        let caps = ServerCapabilities {
            definition_provider: Some(Value::Bool(false)),
        };
        assert!(!caps.supports_definition());
    }

    #[test]
    fn server_capabilities_definition_provider_object() {
        let caps = ServerCapabilities {
            definition_provider: Some(json!({"workDoneProgress": true})),
        };
        assert!(
            caps.supports_definition(),
            "object form indicates support per the spec"
        );
    }

    #[test]
    fn server_capabilities_no_definition_provider() {
        let caps = ServerCapabilities {
            definition_provider: None,
        };
        assert!(!caps.supports_definition());
    }
}
