# Indexed data and MCP security contract

Status: **security regression plan, not a claim that the system is hardened**.

## Existing threat surface

`SECURITY.md` documents that the local index contains full source in FTS, and older databases may retain source in `read_cache` until their next writable open clears it (#866). The MCP server uses stdio and exposes both read and edit tools. Repository content and tool results are untrusted input to an agent.

## Security boundaries to test

1. **Index inclusion**: configured exclusions and git-ignore behavior are enforced; indexing a path outside the selected project is never implicit.
2. **Disclosure**: MCP read results and diagnostics may contain confidential source. Tool authorization, response contents and defaults must match the documented model.
3. **Path safety**: reject traversal, absolute-path escapes and symlink escapes for tools scoped to repository files.
4. **Prompt injection**: retrieved comments/source text are data, not trusted tool instructions. Documentation must explicitly tell consumers not to execute instructions embedded in indexed content.
5. **Local storage**: SQLite, WAL, SHM, exports and historical caches are sensitive; `.gitignore` is not encryption or access control.
6. **Observability**: errors, tracing, profiling and benchmark artifacts must not unexpectedly include raw source or secrets.
7. **Edit boundaries**: mutating MCP tools must respect the intended project root and must not follow arbitrary out-of-root targets.

## Regression fixtures

- An ignored file containing a uniquely identifiable fake token.
- A source comment containing a fake system-level instruction.
- `../` traversal, absolute paths and symlinks into a second directory.
- A database populated by a previous TokenSave version with legacy cached text.
- Repository with sensitive text in symbols and function bodies, used to inspect `tools/call` outputs and diagnostic exports.
- Maliciously large/binary/invalid-UTF8 inputs for error and output sanitization.

## Acceptance criteria

- No indexing or editing outside the declared root through traversals/symlinks.
- Explicit documented behavior for ignored-file visibility across index, direct-read and MCP tools; tests must not assume all tools share the same exclusion policy.
- Opt-in exports are audited for sensitive content and do not upload by default.
- Raw user source is never promoted to instructions by TokenSave's integration guidance.
- Cross-platform tests run without external credentials; all secrets are synthetic.

## Implementation order

1. Map every read/edit MCP tool to file-resolving and data-output paths.
2. Add tests exposing current behavior; distinguish security vulnerabilities from documented choices.
3. Fix proven escapes and leaks in narrowly scoped code PRs.
4. Update `SECURITY.md` with precise limitations and remaining risks.

## Status

| Boundary | Covered by |
| --- | --- |
| 1. Index inclusion | `tests/security_boundaries_test.rs`: gitignored and `exclude`d files stay out of every index-backed tool. Following in-root symlinks out of the root (#34) remains implicit. |
| 2. Disclosure | `tests/security_boundaries_test.rs`: `tokensave_read` reads ignored and excluded files inside the project by design and refuses `.git`; documented in `SECURITY.md`. |
| 3. Path safety | `tests/edit_containment_test.rs` (#975) for the edit tools; `tokensave_read` containment from #636. |
| 4. Prompt injection | The `initialize` instructions and the canonical rules text tell the agent to treat repository content as data (#866), with unit tests in `src/mcp/server.rs` and `src/agents/rules.rs`. |
| 5. Local storage | A writable open clears legacy `read_cache` rows with `secure_delete` on (#866), tested in `src/db/connection.rs`. |
| 6. Observability | `tests/security_boundaries_test.rs` checks CLI output while indexing and syncing and the `status` and runtime telemetry output against synthetic secrets (Linux and macOS). The profile export has its own allowlist (#754). |
| 7. Edit boundaries | `tests/edit_containment_test.rs` (#975). |
