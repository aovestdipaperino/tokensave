# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-03-24

### Added
- `claude-install` CLI command — configures Claude Code integration (MCP server, permissions, hook, CLAUDE.md rules) in a single step, replacing the bash `setup.sh` script
- `hook-pre-tool-use` hidden CLI command — cross-platform PreToolUse hook handler written in pure Rust (no bash/jq dependency), blocks Explore agents and exploration-style prompts

### Removed
- Embedded bash hook script — the hook is now a native Rust subcommand

## [1.1.0] - 2026-03-24

### Added
- `tokensave files` CLI command — list indexed files with `--filter` (directory prefix), `--pattern` (glob), and `--json` output
- `tokensave affected` CLI command — BFS through file dependency graph to find test files impacted by source changes; supports `--stdin` (pipe from `git diff --name-only`), `--depth`, `--filter`, `--json`, `--quiet`
- `tokensave_files` MCP tool — file listing with path/pattern filtering, flat or grouped-by-directory output
- `tokensave_affected` MCP tool — find affected test files via file-level dependency traversal
- Graceful shutdown handler for MCP server — persists tokens-saved counter, checkpoints SQLite WAL, and logs session summary on SIGINT/SIGTERM
- `Database::checkpoint()` method for WAL cleanup on shutdown

## [1.0.1] - 2026-03-24

### Changed
- Increased ANSI logo size by 25%

## [1.0.0] - 2026-03-24

### Changed
- **Renamed project from `token-codegraph` to `tokensave`**
- Crate name: `tokensave` (was `token-codegraph`)
- Binary name: `tokensave` (was `codegraph`)
- Data directory: `.tokensave/` (was `.codegraph/`)
- MCP tool prefix: `tokensave_*` (was `codegraph_*`)
- Version bump to 1.0.0

### Added
- TypeScript/JavaScript language support (.ts, .tsx, .js, .jsx)
- Python language support (.py)
- C language support (.c, .h)
- C++ language support (.cpp, .hpp, .cc, .cxx, .hh)
- Kotlin language support (.kt, .kts)
- Dart language support (.dart)
- C# language support (.cs)
- Pascal language support (.pas, .pp, .dpr)
- Legacy `.codegraph/` directory detection with migration warning
- CHANGELOG.md for tracking version history

## [0.6.0]

### Added
- Scala language support (.scala, .sc)

### Fixed
- Self-animating spinner with cursor hiding and path truncation
- Show each language as its own cell in status table

### Changed
- Show indexed languages in status, fix multi-language file discovery

## [0.5.2]

### Changed
- Update repo URLs after GitHub rename to tokensave
- Rename crate to tokensave for crates.io

## [0.5.1]

### Added
- Compact bordered table for status output

## [0.5.0]

### Added
- Java language support (.java)
- Go language support (.go)
- ANSI logo and crates.io readiness

### Changed
- NASA rules compliance improvements

## [0.4.2]

### Added
- Versioned DB migration system with exclusive locking

### Fixed
- Create metadata table on open for existing databases

## [0.4.1]

### Added
- Show version number in tokensave status
- Persist tokens-saved counter to database
- Show indexed token count in tokensave status

### Changed
- Update dependencies

## [0.4.0]

### Added
- Initial Rust language support (.rs)
- Replace rusqlite with native libsql (Turso) crate
- Sync progress spinner and post-commit hook
- Prompt to create index when invoked with no command
- Install section with setup script and hooks

### Changed
- Replace `index` command with `sync --force`

## [0.3.0]

### Added
- MCP tool call logging to stderr
- Merge init and index into a single command

### Fixed
- Harden MCP inputs and prevent path traversal

## [0.2.0]

### Added
- Go extractor with deep extraction support
- Java extractor with deep extraction support
- LanguageExtractor trait and LanguageRegistry for multi-language dispatch
- Runtime stats tracking to MCP server
- Homebrew release workflow

### Fixed
- Sanitize FTS5 search queries to handle special characters
- Address code review findings (UTF-8 safety, FK violations, stats accuracy)

## [0.1.0]

### Added
- MCP server (JSON-RPC 2.0 over stdio)
- CLI interface and TokenSave orchestrator
- Vector embeddings for semantic search
- Context builder for AI-ready code graph context
- Incremental sync for detecting file changes
- Graph traversal and query operations
- Reference resolution module
- Tree-sitter Rust extraction module
- libsql database layer with full CRUD operations
- Configuration module with glob-based file filtering
- Core types and error handling scaffold
