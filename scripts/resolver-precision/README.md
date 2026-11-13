# Resolver precision harness

This development-only harness compares the reference sites and rename confidence
classes reported by tokensave with references returned by three language-server
oracles. It does not add a language-server dependency to the tokensave binary.

The inputs are deliberately small, bundled fixtures. `manifest.json` pins every
fixture by a SHA-256 over its relative paths and bytes, and pins these servers:

- rust-analyzer from the Rust 1.99.0 toolchain (build `b940084d`, 2026-09-28);
- typescript-language-server 6.0.3 using TypeScript/tsserver 6.0.3;
- Pyright 1.1.414.

Install exactly those versions into an isolated tools directory. For example,
install the pinned toolchain's rust-analyzer component, then use an npm prefix
that is not part of this repository:

```text
rustup component add rust-analyzer --toolchain 1.99.0
npm install --prefix /private/resolver-tools \
  typescript-language-server@6.0.3 typescript@6.0.3 pyright@1.1.414
```

Put that prefix's executable directory and the pinned rust-analyzer directory on
`PATH`. Installation is the only networked step. A measurement run performs no
downloads, clones, package installation, or dependency resolution; the Rust
fixture has an empty workspace and the TypeScript and Python fixtures have no
dependencies.

Build tokensave and run:

```text
cargo build --locked
python3 scripts/resolver-precision/resolver_precision.py \
  --tokensave target/debug/tokensave \
  --output benchmarks/resolver-precision/baseline.json \
  --markdown /tmp/resolver-precision.md
```

Use `--language rust` (repeatable) for a subset. Existing output files are
replaced. The JSON retains each raw oracle response after path normalization and
the complete tokensave rename plan. The Markdown file is generated from the same
aggregates. The committed narrative baseline is
`docs/RESOLVER-PRECISION-BASELINE.md`; update it only from a complete three-
language run and retain the generated JSON under `benchmarks/resolver-precision/`.

Run the matching tests with:

```text
python3 -m unittest discover -s scripts/resolver-precision
```

## Method and limits

Each probe points at a definition. The harness asks the language server for the
definition and all references, then asks the public `tokensave tool rename
--dry-run --json` path for the same unique symbol. Locations are normalized to
project-relative paths, one-based lines, and one-based UTF-8 byte columns.
Definitions are excluded from precision and recall. Results are micro-averaged
by language and the probe's expected edge kind (`calls` or `uses`). Rename-class
precision tests whether each located `exact`, `heuristic`, `ambiguous`, or
`text_only` site appears in the oracle's reference set. Class share is the
class's located-site count divided by all located rename sites for that language.

The language servers are comparison oracles, not perfect semantic truth. They
can have bugs, version-specific behavior, different workspace assumptions, and
different ideas about dynamic references. The fixtures favor statically obvious
calls and type uses, so the figures do not generalize to arbitrary repositories.
The harness measures six named symbols, not the full population of graph edges.
An absent class has `null` precision rather than an assumed perfect score. LSP
positions are UTF-16; the harness converts them to tokensave's UTF-8 byte columns.
It does not measure macro-generated, generated, dependency, or unindexed code.
