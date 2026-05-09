// Rust guideline compliant 2025-10-17
//! Bridges `UnresolvedRef` instances to graph edges via LSP
//! `textDocument/definition` requests.
//!
//! The flow:
//!
//! 1. The sync pipeline collects `UnresolvedRef`s across all extracted files.
//! 2. `LspResolver::resolve_all` partitions them by language.
//! 3. For each ref whose language has a running LSP client, the resolver
//!    sends `textDocument/definition` and looks the response up in a
//!    `(file, line) -> node_id` index built from the project's nodes.
//! 4. Successful lookups become `ResolvedRef`s with `resolved_by = "lsp"`.
//! 5. Refs the LSP couldn't resolve (no client, target outside project,
//!    server timeout, no definition) flow back to the caller for the
//!    heuristic resolver to retry.
//!
//! In this 5.0 slice the manager is allowed to be empty (no clients
//! registered yet) — the resolver is then a pass-through that returns every
//! ref as still-unresolved. This lets the integration with sync land before
//! any specific adapter does.

use std::collections::HashMap;
use std::path::Path;

use crate::errors::Result;
use crate::lsp::client::LspClient;
use crate::lsp::manager::LspManager;
use crate::lsp::protocol::{
    parse_definition_response, path_to_uri, DefinitionParams, DidOpenTextDocumentParams, Position,
    TextDocumentIdentifier, TextDocumentItem,
};
use crate::types::{Node, ResolutionResult, ResolvedRef, UnresolvedRef};

/// Resolves unresolved references through running LSP servers.
pub struct LspResolver<'a> {
    manager: &'a LspManager,
    /// Index: (relative_file_path, start_line) -> node_id.
    /// Built once per resolve_all call. Lookups go from a 0-based LSP
    /// position to a 1-based start_line (LSP positions are zero-based; node
    /// `start_line` is 1-based, so the lookup adds 1).
    node_index: HashMap<(String, u32), String>,
    /// Track which (language, file) pairs we've already opened so we don't
    /// send duplicate `didOpen` notifications.
    opened: tokio::sync::Mutex<std::collections::HashSet<(String, String)>>,
}

impl<'a> LspResolver<'a> {
    /// Build a resolver from the project's full node list. The index is
    /// keyed by relative path (matching `Node::file_path`) so the URI
    /// returned by the server has to be normalised to the same form.
    pub fn from_nodes(manager: &'a LspManager, nodes: &[Node]) -> Self {
        let mut node_index: HashMap<(String, u32), String> = HashMap::new();
        for n in nodes {
            // Stash by both 0-based and 1-based-shifted-down keys so a
            // single-row miss isn't fatal under the (rare) symbols whose
            // attrs_start_line precedes start_line. The resolver's hot
            // path uses the 0-based form computed from the LSP position.
            node_index
                .entry((n.file_path.clone(), n.start_line))
                .or_insert_with(|| n.id.clone());
        }
        Self {
            manager,
            node_index,
            opened: tokio::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Number of `(file, start_line)` entries the resolver indexed. Useful
    /// in tests and diagnostics.
    pub fn index_size(&self) -> usize {
        self.node_index.len()
    }

    /// Drive every ref through LSP `textDocument/definition` and partition
    /// the results into resolved / still-unresolved buckets.
    ///
    /// When the manager has no clients registered, this is a pass-through:
    /// every ref is returned untouched in the `unresolved` field. That is
    /// the path taken on systems where no Phase 1 LSP server is installed.
    pub async fn resolve_all(&self, refs: &[UnresolvedRef]) -> Result<ResolutionResult> {
        let total = refs.len();

        if !self.manager.is_active() {
            return Ok(ResolutionResult {
                resolved: Vec::new(),
                unresolved: refs.to_vec(),
                total,
                resolved_count: 0,
            });
        }

        let mut resolved: Vec<ResolvedRef> = Vec::new();
        let mut unresolved: Vec<UnresolvedRef> = Vec::new();

        for uref in refs {
            match self.try_resolve_one(uref).await {
                Ok(Some(r)) => resolved.push(r),
                Ok(None) | Err(_) => unresolved.push(uref.clone()),
            }
        }

        let resolved_count = resolved.len();
        Ok(ResolutionResult {
            resolved,
            unresolved,
            total,
            resolved_count,
        })
    }

    /// Resolve a single ref. Returns `Ok(Some(_))` on success, `Ok(None)`
    /// when the LSP couldn't determine a definition (server returned null
    /// or pointed outside the project), and `Err` on transport failure.
    async fn try_resolve_one(&self, uref: &UnresolvedRef) -> Result<Option<ResolvedRef>> {
        let abs_path = self.manager.project_root().join(&uref.file_path);
        let lang = match LspManager::language_for_path(&abs_path) {
            Some(l) => l,
            None => return Ok(None), // unknown extension — heuristic only
        };
        let client = match self.manager.client_for_language(lang) {
            Some(c) => c,
            None => return Ok(None), // no LSP for this language
        };

        // didOpen, exactly once per (lang, file). Ignored on subsequent calls.
        self.ensure_open(&client, lang, &abs_path, &uref.file_path)
            .await?;

        // LSP positions are 0-based; UnresolvedRef line/column are 1-based
        // (extracted from tree-sitter's 1-based row/col output).
        let position = Position {
            line: uref.line.saturating_sub(1),
            character: uref.column.saturating_sub(1),
        };
        let params = DefinitionParams {
            text_document: TextDocumentIdentifier {
                uri: path_to_uri(&abs_path),
            },
            position,
        };

        let raw: serde_json::Value = client.request("textDocument/definition", params).await?;
        let targets = parse_definition_response(&raw);

        for target in targets {
            let target_path = match uri_to_relative_path(&target.uri, self.manager.project_root()) {
                Some(p) => p,
                None => continue, // outside project
            };
            // LSP positions are 0-based; node start_line is 1-based.
            let target_line = target.range.start.line + 1;
            // First try exact match on start_line; if the server returns a
            // selection range that lands on a body line rather than the def
            // line, try ±1 as well — this matches what most servers do.
            for delta in [0i32, -1, 1, -2, 2] {
                let key_line = (target_line as i32 + delta).max(0) as u32;
                if let Some(node_id) = self.node_index.get(&(target_path.clone(), key_line)) {
                    return Ok(Some(ResolvedRef {
                        original: uref.clone(),
                        target_node_id: node_id.clone(),
                        confidence: 0.99,
                        resolved_by: "lsp".to_string(),
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn ensure_open(
        &self,
        client: &LspClient,
        lang: &str,
        abs_path: &Path,
        rel_path: &str,
    ) -> Result<()> {
        let key = (lang.to_string(), rel_path.to_string());
        {
            let mut opened = self.opened.lock().await;
            if !opened.insert(key) {
                return Ok(());
            }
        }

        let text = std::fs::read_to_string(abs_path).unwrap_or_default();
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: path_to_uri(abs_path),
                language_id: lang.to_string(),
                version: 1,
                text,
            },
        };
        client.notify("textDocument/didOpen", params).await
    }
}

/// Convert a `file://` URI to a project-relative path. Returns `None` if
/// the URI is not under `project_root` or the percent-encoding is invalid.
fn uri_to_relative_path(uri: &str, project_root: &Path) -> Option<String> {
    let path_part = uri.strip_prefix("file://")?;
    let decoded = percent_decode(path_part)?;
    let abs = std::path::PathBuf::from(decoded);
    let rel = abs.strip_prefix(project_root).ok()?;
    Some(rel.to_string_lossy().to_string())
}

/// Decode `%HH` escapes back to bytes. Returns `None` on malformed input.
/// Tokensave only accepts UTF-8 paths so the result is converted via
/// `String::from_utf8`.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            let hi = hex_digit(bytes[i + 1])?;
            let lo = hex_digit(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EdgeKind, NodeKind, Visibility};
    use std::path::PathBuf;

    fn sample_node(id: &str, file: &str, line: u32) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Function,
            name: id.to_string(),
            qualified_name: format!("crate::{id}"),
            file_path: file.to_string(),
            start_line: line,
            attrs_start_line: line,
            end_line: line + 5,
            start_column: 0,
            end_column: 1,
            signature: None,
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            updated_at: 0,
        }
    }

    fn sample_uref(name: &str, file: &str, line: u32) -> UnresolvedRef {
        UnresolvedRef {
            from_node_id: "src-node".to_string(),
            reference_name: name.to_string(),
            reference_kind: EdgeKind::Calls,
            line,
            column: 1,
            file_path: file.to_string(),
        }
    }

    #[test]
    fn percent_decode_round_trip() {
        assert_eq!(percent_decode("hello%20world").unwrap(), "hello world");
        assert_eq!(percent_decode("/usr/local/bin").unwrap(), "/usr/local/bin");
        assert!(percent_decode("invalid%XY").is_none());
    }

    #[test]
    fn uri_to_relative_path_strips_project_root() {
        let root = PathBuf::from("/tmp/proj");
        let rel = uri_to_relative_path("file:///tmp/proj/src/foo.rs", &root).unwrap();
        assert_eq!(rel, "src/foo.rs");
    }

    #[test]
    fn uri_to_relative_path_outside_project_returns_none() {
        let root = PathBuf::from("/tmp/proj");
        assert!(uri_to_relative_path("file:///etc/passwd", &root).is_none());
    }

    #[test]
    fn uri_to_relative_path_handles_percent_escapes() {
        let root = PathBuf::from("/tmp/my proj");
        let rel = uri_to_relative_path("file:///tmp/my%20proj/src/foo.rs", &root).unwrap();
        assert_eq!(rel, "src/foo.rs");
    }

    #[tokio::test]
    async fn resolve_all_is_pass_through_when_manager_inactive() {
        let manager = LspManager::new(PathBuf::from("/tmp/proj"));
        let nodes = vec![sample_node("foo", "src/lib.rs", 10)];
        let resolver = LspResolver::from_nodes(&manager, &nodes);
        let refs = vec![sample_uref("foo", "src/lib.rs", 12)];
        let result = resolver.resolve_all(&refs).await.unwrap();
        assert_eq!(result.resolved_count, 0);
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.total, 1);
        assert_eq!(result.unresolved[0].reference_name, "foo");
    }

    #[test]
    fn from_nodes_indexes_by_file_and_start_line() {
        let manager = LspManager::new(PathBuf::from("/tmp/proj"));
        let nodes = vec![
            sample_node("foo", "src/a.rs", 10),
            sample_node("bar", "src/b.rs", 20),
            sample_node("baz", "src/a.rs", 30),
        ];
        let resolver = LspResolver::from_nodes(&manager, &nodes);
        assert_eq!(resolver.index_size(), 3);
    }
}
