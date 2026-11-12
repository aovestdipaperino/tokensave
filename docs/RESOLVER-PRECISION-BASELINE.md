# Resolver Precision Baseline

This report records the first development-only comparison between tokensave's
name-based reference resolver and pinned language-server oracles (Refs #921,
part of #916). It does not claim that the language servers are perfect ground
truth, and it does not change tokensave's runtime behavior.

## Recorded run

The run was made on Windows 11 on 2026-11-11 with the current `master` binary
at `e920051`, using:

- rust-analyzer 0.3.3113, standalone release commit `e5e7830`;
- typescript-language-server 6.0.3 with TypeScript/tsserver 6.0.3;
- Pyright 1.1.414;
- the six bundled fixture probes pinned by `scripts/resolver-precision/manifest.json`.

The complete normalized inputs, language-server responses, tokensave rename
plans and aggregates are retained in
`benchmarks/resolver-precision/baseline.json`. The command was:

```text
python scripts/resolver-precision/resolver_precision.py \
  --tokensave target/debug/tokensave.exe \
  --output benchmarks/resolver-precision/baseline.json \
  --markdown /tmp/resolver-precision.md
```

The manifest SHA-256 for this run was
`5e930d103e77e0f19f6f090b60e51086d5b2cbd3691df246e390b340f60dcd70`.

## References by language and edge kind

| Language | Edge kind | Predicted | Oracle | TP | FP | FN | Precision | Recall |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Python | calls | 2 | 2 | 2 | 0 | 0 | 1.0 | 1.0 |
| Python | uses | 3 | 3 | 3 | 0 | 0 | 1.0 | 1.0 |
| Rust | calls | 2 | 2 | 2 | 0 | 0 | 1.0 | 1.0 |
| Rust | uses | 3 | 3 | 3 | 0 | 0 | 1.0 | 1.0 |
| TypeScript | calls | 2 | 2 | 2 | 0 | 0 | 1.0 | 1.0 |
| TypeScript | uses | 2 | 2 | 2 | 0 | 0 | 1.0 | 1.0 |

## Rename confidence classes

`Sites` excludes the two definitions per language. `Share` is the share of
located, non-definition tokensave sites for that language. A class with no
sites has `n/a` precision rather than an assumed perfect score.

| Language | Class | Sites | Share | Oracle | TP | FP | Precision |
|---|---|---:|---:|---:|---:|---:|---:|
| Python | ambiguous | 0 | 0.0 | 5 | 0 | 0 | n/a |
| Python | exact | 2 | 0.4 | 5 | 2 | 0 | 1.0 |
| Python | heuristic | 0 | 0.0 | 5 | 0 | 0 | n/a |
| Python | text_only | 3 | 0.6 | 5 | 3 | 0 | 1.0 |
| Rust | ambiguous | 0 | 0.0 | 5 | 0 | 0 | n/a |
| Rust | exact | 4 | 0.8 | 5 | 4 | 0 | 1.0 |
| Rust | heuristic | 0 | 0.0 | 5 | 0 | 0 | n/a |
| Rust | text_only | 1 | 0.2 | 5 | 1 | 0 | 1.0 |
| TypeScript | ambiguous | 0 | 0.0 | 4 | 0 | 0 | n/a |
| TypeScript | exact | 4 | 1.0 | 4 | 4 | 0 | 1.0 |
| TypeScript | heuristic | 0 | 0.0 | 4 | 0 | 0 | n/a |
| TypeScript | text_only | 0 | 0.0 | 4 | 0 | 0 | n/a |

## Interpretation and limits

All 14 reference sites in these fixtures matched the corresponding oracle.
That is a smoke-test baseline, not evidence of general 100% precision or
recall. The fixtures intentionally contain small, statically obvious calls and
type uses. They do not cover shadowing, overloaded same-named symbols, dynamic
dispatch, macros, dependencies, generated code or unindexed files. No
heuristic or ambiguous rename site occurred, so this run says nothing about
those classes' precision.

Language servers can themselves be wrong and can change between versions.
Fixture selection strongly biases the result. See
`scripts/resolver-precision/README.md` for the full method, offline setup,
position normalization and additional limitations.
