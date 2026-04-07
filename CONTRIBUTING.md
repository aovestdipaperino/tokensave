# Contributing to tokensave

Thanks for your interest in contributing! This guide covers everything you need to get started.

## Getting Started

```bash
git clone https://github.com/aovestdipaperino/tokensave.git
cd tokensave
cargo build
cargo test
```

Requires **Rust 1.70+** (edition 2021).

## Project Structure

```
src/
  extraction/    Language-specific extractors (tree-sitter based)
  db/            Database layer (libSQL)
  graph/         Knowledge graph queries and traversal
  mcp/           MCP server (tools + handlers)
  context/       Context builder for AI-ready output
  resolution/    Cross-file reference resolution
  sync.rs        Incremental sync engine
  daemon.rs      Background daemon
  main.rs        CLI entry point
tests/           Integration tests (one per module/language)
tests/fixtures/  Sample source files for extraction tests
vendor/          Vendored tree-sitter grammars
docs/            Design docs and guides
```

## Feature Flags

tokensave supports 31 languages via feature flags:

| Feature | Languages |
|---------|-----------|
| `lite` (default subset) | Rust, Go, Java, Scala, TypeScript/JS, Python, C, C++, Kotlin, C#, Swift |
| `medium` | +Dart, Pascal, PHP, Ruby, Bash, Protobuf, PowerShell, Nix, VB.NET |
| `full` (default) | +Lua, Zig, Obj-C, Perl, Batch, Fortran, COBOL, MSBasic2, GW-BASIC, QBasic |

Build with fewer languages for faster compile times during development:

```bash
cargo build --no-default-features --features lite
cargo test --no-default-features --features lite
```

## Making Changes

1. **Fork and branch** from `master` for stable changes, `beta` for experimental features.
2. **Write tests.** Every extraction change should have a corresponding test in `tests/`. Follow the existing pattern: create a fixture in `tests/fixtures/` and assert on extracted nodes/edges.
3. **Run the full test suite** before submitting:
   ```bash
   cargo test
   ```
4. **Format your code** with the standard Rust toolchain:
   ```bash
   cargo fmt
   cargo clippy
   ```

## Adding a New Language Extractor

1. Add a tree-sitter grammar dependency (or vendor it under `vendor/`).
2. Create `src/extraction/{lang}_extractor.rs` implementing the `Extractor` trait.
3. Register it in the `LanguageRegistry` with a feature flag (e.g., `lang-{name}`).
4. Add a fixture file `tests/fixtures/sample.{ext}` and a test file `tests/{lang}_extraction_test.rs`.
5. Update the feature flag tables in `Cargo.toml` and this document.

## Running Specific Tests

```bash
# All tests for a specific language
cargo test --test rust_extraction_test

# A single test by name
cargo test test_find_stale_files

# Only sync-related tests
cargo test sync
```

## Commit Messages

Follow conventional commit style:

```
fix: handle UTF-16 encoded files in sync
feat: add Dart annotation extraction
refactor: simplify reference resolver lookup
```

Keep the first line under 72 characters. Add a body explaining *why* if the change isn't obvious.

## Pull Requests

- Target `master` for bug fixes and stable features.
- Target `beta` for experimental or breaking changes.
- Keep PRs focused — one logical change per PR.
- Include test coverage for new behavior.
- Update `CHANGELOG.md` under an `[Unreleased]` section.

## Reporting Issues

Open an issue at https://github.com/aovestdipaperino/tokensave/issues with:

- tokensave version (`tokensave --version`)
- OS and architecture
- Steps to reproduce
- Expected vs. actual behavior

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). Be respectful and constructive.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
