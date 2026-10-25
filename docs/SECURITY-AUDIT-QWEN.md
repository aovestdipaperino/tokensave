# Security audit — `tokensave`

- **Repo:** `aovestdipaperino/tokensave` (origin)
- **Date:** 2026-10-03
- **Auditor:** Qwen (via Plank)

## Scope

Rust crate — tree-sitter indexing → SQLite/libSQL graph → CLI + stdio MCP server.
Every client-/input-reachable sink was traced: SQL construction, subprocess spawning,
path handling, the worker-token auth, TLS/telemetry, and the MCP transport.

**Threat model that matters here:** the MCP client is a *local* AI agent (same user,
stdio-only). The genuinely untrusted inputs are (a) **source files being indexed** and
(b) **the MCP client's tool arguments**. Findings are rated against that.

## Findings

### 1. MEDIUM — `tokensave_read` reads arbitrary absolute paths for the primary project (latent sandbox escape)

`src/mcp/tools/handlers/info.rs:1389-1423`

```rust
let mut abs_path = if Path::new(file).is_absolute() { PathBuf::from(file) }
                   else { project_root.join(&rel_path) };
if cg.db().is_read_only() {            // ← containment ONLY when read-only
    ... if !canonical_path.starts_with(&canonical_root) { return Err(...) }
}
```

The `starts_with` containment check is gated on `is_read_only()`. Federated
`graph_root`s are opened `open_read_only` (contained), but the **primary project** is
opened read-write (`serve.rs:7` → `TokenSave::open`), so the check is skipped. A client
passing `file: "~/.ssh/id_rsa"` expanded to an absolute path or `file: "../../.ssh/id_rsa"` gets the raw
contents.

- **Why it's not Critical:** in the typical local setup the agent already has full FS
  access, so no privilege *gain*.
- **Why it's still Medium:** it's a **sandbox escape**. Any agent host that sandboxes
  the agent's *direct* file access to the workspace (an increasingly common hardening)
  has that sandbox defeated, because the MCP server process is unsandboxed. The
  asymmetry with federated roots strongly suggests the containment was meant to be
  universal and the primary case was missed.
- **Fix:** apply the canonicalize + `starts_with` check unconditionally (or at least for
  any absolute / escaping path), not only when `is_read_only()`.

### 2. LOW — MCP stdin line reader has no size bound (memory DoS)

`src/mcp/transport.rs:145,153,167`

`StdioTransport` uses `tokio::io::BufReader::stdin().lines()` → `tokio::io::Lines`,
which has **no per-line cap** (unlike `tokio_util::codec::LinesCodec`'s 8 KB default).
A single arbitrarily long line is buffered fully in memory before parsing. A buggy or
hostile host could OOM the server. Low because the client is a same-user local process.

- **Fix:** bound the reader (e.g. `FramedRead` with a max-size codec, or a manual
  length check).

### 3. LOW — git hook snippets embed the binary path unquoted (self-inflicted shell injection)

`src/agents/hooks.rs:448-454, 457-461, 549-556`

```sh
{bin} sync >/dev/null 2>&1 &          # bin = current_exe(), unquoted
```

All three snippets (post-commit / post-merge / post-checkout) interpolate the resolved
binary path with no shell quoting. If the install path contains shell metacharacters
(`~/My;Tools/tokensave`, `$(…)`, backticks), the generated hook runs arbitrary commands
on every git op. **Low** because the path is the user's own install location
(self-inflicted; the code already handles the *spaces* case for the JSON hook config in
#81/#146, just not the shell form).

- **Fix:** shell-quote `bin` (e.g. `shlex`-style single-quoting) in the snippet.

## Exploitability review

A second pass checked each finding against the code to decide whether an attacker can
actually use it. Only finding 1 holds up, and it is stronger than "latent".

### Finding 1 is exploitable, through the agent

The attacker never talks to tokensave directly: the server speaks stdio to a local
process. The route in is prompt injection. A file in a cloned repository, a fetched web
page, an issue body or a dependency's README tells the agent to call `tokensave_read`
on a credentials file (`~/.ssh/id_rsa`, `~/.aws/credentials`, `~/.config/gh/hosts.yml`).
An absolute path, a `../` escape, or a symlink committed inside the repository and
pointing outside it all work, because the primary project never runs the containment
check.

Two details turn this from a theoretical gap into a working bypass:

- **The call is pre-approved.** `tokensave install` writes `mcp__tokensave__*` (or the
  full per-tool list) into Claude Code's `permissions.allow`
  (`src/agents/mod.rs`, `install_tool_perms`). A direct `Read` of a home-directory
  secret from inside a project normally hits a permission prompt or a deny rule; the
  same read through `tokensave_read` runs silently. tokensave becomes a way around the
  user's own file-access policy, whatever the host enforces on its built-in tools.
- **The secret is copied to disk.** On a read-write database the read cache is enabled
  (`cache_enabled = !is_read_only()`), so `read_cache::put` stores the full body of the
  out-of-tree file in `.tokensave/tokensave.db`, keyed by the stripped path
  (`etc/passwd`). The secret now lives in a project-local file that is easy to copy,
  back up or share without anyone noticing.

Exfiltration still needs a second step, since the content lands in the agent's
context: a web request, a commit, a PR or issue comment. Whether that step needs
approval depends on the host, so the end-to-end risk varies, but the read itself is
silent.

**Status:** fixed in the working tree by making the containment check unconditional in
`handle_read`. Regression test: `primary_read_rejects_paths_outside_project_root` in
`tests/mcp_server_test.rs` covers the absolute path, the `../` escape and the symlink,
checks that the secret never appears in the response and that no read-cache row is
written, and confirms a normal in-project read still succeeds. The test fails against
the unfixed code (the secret is returned) and passes with the fix.

### Finding 2 is not exploitable

Only the process that spawned the server can write to its stdin, and that is the agent
host running as the same user. Prompt injection cannot realistically make a model emit
a multi-gigabyte tool argument, because output token limits stop it long before. The
worst outcome is a misbehaving host crashing its own tokensave server, which it can
restart. A line cap is reasonable robustness work, not a security fix.

### Finding 3 is not exploitable

The binary path comes from `which_tokensave()`, which reads `std::env::current_exe()`.
Only someone who controls where the binary is installed can put shell metacharacters in
it, and anyone with that control can replace the binary outright, so command injection
gains them nothing. The practical effect is a plain bug: an install path containing a
space breaks the hooks. Double-quoting `{bin}` fixes the space case; it does not stop
`$(...)` or backticks, which only matters for correctness, since there is no security
boundary to cross.

## Reviewed and clean (the surface was covered)

- **SQL injection — clean.** `src/db/` builds SQL only via `?{n}` placeholder lists +
  `params_from_iter`; the only `format!`-into-SQL sites are `PRAGMA user_version = {int}`,
  `PRAGMA table_info({table})` (single literal caller `"files"`), and `cache_size`/
  `mmap_size` (derived from a `u64`). `graph_root`/`graph_branch` select a
  *file/connection*, never splice into SQL.
- **Worker-token auth — solid.** `src/extraction_worker.rs`: 32-byte `getrandom` token,
  two-factor (env + first-32-bytes-on-stdin), env scrubbed immediately, **constant-time**
  compare (`slices_eq`, XOR-accumulate, no early exit).
- **Self-update — well hardened.** `src/upgrade.rs`: SHA256 verified *before* extraction,
  fail-closed on missing/absent sums (#525), `TempDir` with `0700` + exclusive create
  (defeats planted-symlink in shared `/tmp`), atomic rename install.
- **Branch → DB path — protected.** `src/branch.rs`: `sanitize_branch_name` strips `.`/`/`,
  a tracked-branch metadata lookup, *and* a `canonicalize` + `starts_with` containment
  check.
- **TLS — sound.** `src/cloud.rs`: `rustls` platform-verifier (WebPKI fallback only
  *narrows* trust), no `danger_accept_invalid_certs`.
- **Telemetry — minimal.** The only network POST in the crate sends
  `{"amount": <token count>}`; no secrets, paths, or file contents leave the machine.
- **Destructive FS ops — contained.** The one `remove_dir_all` (`commands.rs:340`)
  targets `<project_root>/.tokensave` behind a `go!` confirmation.
- **`unsafe` — limited & benign:** `getppid()` FFI, `memmap2` maps, tree-sitter grammar
  pointers. Parser crashes are contained by the worker-subprocess isolation.
- **Output DoS — bounded:** MCP tools cap `limit` and responses truncate at
  `MAX_RESPONSE_CHARS = 15_000`.

## Bottom line

Genuinely security-conscious codebase — the high-value surfaces (update supply chain,
worker auth, SQL, TLS, telemetry) are all properly hardened. Findings 2 and 3 are
hardening gaps with no security boundary behind them. **Finding 1 is exploitable**
through prompt injection: because tokensave's tools are pre-approved, it silently
bypasses the host's file-access permissions, and it caches the stolen file in the
project database. The fix is small and is in place.