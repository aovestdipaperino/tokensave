# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in tokensave, please report it responsibly:

- **Email:** security@tokensave.dev
- **GitHub:** Open a [private security advisory](https://github.com/aovestdipaperino/tokensave/security/advisories/new)

Please do **not** open a public issue for security vulnerabilities. We aim to acknowledge reports within 48 hours and provide a fix or mitigation plan within 7 days.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 3.4.x   | Yes       |
| 4.x-beta | Best-effort |
| < 3.4   | No        |

## Security Model

### What tokensave stores

tokensave builds a **local** code graph stored in a SQLite database (`.tokensave/tokensave.db`) inside your project directory. The database contains:

- Symbol names, signatures, and docstrings
- File paths, sizes, and content hashes
- Call relationships and dependency edges
- FTS5 search index

It does **not** store raw source code. The database is local-only — there is no cloud sync, remote database, or server-side storage.

A second database (`~/.tokensave/global.db`) tracks which projects have been indexed and aggregate token-saved counts. It contains only directory paths and counters.

### Network access

tokensave makes **no inbound network connections**. It never binds a port or listens for traffic. The MCP server communicates exclusively over stdio.

Outbound connections are limited to:

| Destination | Purpose | Auth | Failure mode |
|-------------|---------|------|-------------|
| `api.github.com` | Check for new releases | None (public API) | Silently ignored |
| `github.com` | Download binary during `tokensave upgrade` | None (public releases) | Error shown to user |
| `tokensave-counter.enzinol.workers.dev` | Aggregate token-saved counter | None | Silently ignored |

All best-effort network calls use short timeouts (1-2 seconds) and never block the CLI or MCP server.

### No credentials or secrets

tokensave does not require, store, or transmit any credentials, API keys, tokens, or passwords. All external API calls target public, unauthenticated endpoints.

### MCP server (read-only)

All 37 MCP tools are **read-only** analysis and query operations (marked `readOnlyHint: true`). The MCP server cannot:

- Modify, create, or delete files
- Execute shell commands or user-supplied code
- Access the network on behalf of the AI agent

### Self-update integrity

`tokensave upgrade` downloads pre-built binaries from [GitHub Releases](https://github.com/aovestdipaperino/tokensave/releases). The upgrade process:

- Stops the daemon before replacing the binary
- Downloads from the same release channel (stable/beta) currently installed
- Restarts the daemon after a successful upgrade

**Limitation:** Release artifacts are not currently signed with a cryptographic signature. The integrity guarantee relies on HTTPS transport security and GitHub's release infrastructure.

### Daemon privileges

The background daemon (`tokensave daemon`) runs with **standard user privileges**. It only watches project directories for file changes and triggers incremental syncs.

On Windows, installing the daemon as an autostart service requires a one-time UAC elevation prompt. The elevated operation is limited to service registration — the daemon itself runs as the current user.

### Unsafe code

The codebase contains minimal `unsafe` blocks, limited to Windows API calls for:

- Checking whether the process is running elevated (`OpenProcessToken`, `GetTokenInformation`)
- Launching an elevated process for service installation (`ShellExecuteExW`)

No unsafe code is used on macOS or Linux.

## Best Practices

- Add `.tokensave/` to your `.gitignore` to avoid committing the local database.
- If your project contains sensitive code, be aware that the database stores symbol names and signatures (though not source code).
- Keep tokensave updated (`tokensave upgrade`) to receive security fixes.
- Review the [CHANGELOG](CHANGELOG.md) before upgrading to understand what changed.

## Scope

The following are **not** security issues:

- The aggregate token counter sending a count to the public Cloudflare Worker endpoint (this is documented behavior and contains no identifying information beyond an approximate country derived from IP by Cloudflare)
- The database containing symbol names or file paths from your project (this is core functionality)
- The daemon running in the background after explicit opt-in (`tokensave daemon --enable-autostart`)
