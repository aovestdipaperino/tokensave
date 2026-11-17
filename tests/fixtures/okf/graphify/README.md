# Graphify OKF fixture

`bundle/` is an OKF bundle written by `to_okf_obsidian` in the
[Graphify OKF fork](https://github.com/alderac/graphify-okf) (MIT license) at
commit `ec1252a11233e3aadae69f856e975ae83029f6af`, for the fork's own test
graph `tests/fixtures/extraction.json` with the communities and labels used by
its `tests/test_okf_export.py`.

The files are output from a real run of that pinned exporter on 2026-11-13
using Python 3.14.7, NetworkX 3.6.1 and PyYAML 6.0.3, with only a final newline
added to its two JSON files. This replaced the earlier hand-reconstructed
fixture and corrected its guessed `graphify_dominant_confidence` values. The
committed output includes `.obsidian/graph.json`; tokensave deliberately skips
that hidden directory.

To reproduce and compare the fixture with Graphify output, run
`regenerate.py` with the fork checked out at the commit above and its
dependencies installed:

```bash
python3 tests/fixtures/okf/graphify/regenerate.py /path/to/graphify-okf /tmp/graphify-bundle
diff -r tests/fixtures/okf/graphify/bundle /tmp/graphify-bundle
```

What the fixture shows about Graphify bundles:

- The root `index.md` declares `okf_version: '0.1'` and adds a
  `producer: graphify` key. OKF v0.2 §8 allows only `okf_version` there, so
  `tokensave okf validate` reports `OKF-E004` and the bundle is not
  conformant. `tokensave okf import` still imports it best-effort.
- Concepts carry their source file in `graphify_source_file`, which
  `tokensave_knowledge` matches against indexed project files.
- `index.md` files other than the root have no frontmatter, as OKF requires.
