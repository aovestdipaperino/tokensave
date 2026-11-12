# Graphify OKF fixture

`bundle/` is an OKF bundle in the form written by `to_okf_obsidian` in the
[Graphify OKF fork](https://github.com/alderac/graphify-okf) (MIT license) at
commit `ec1252a11233e3aadae69f856e975ae83029f6af`, for the fork's own test
graph `tests/fixtures/extraction.json` with the communities and labels used by
its `tests/test_okf_export.py`.

The files were **reconstructed by hand** from the exporter source at that
commit, not produced by running Graphify. Two parts of the real output are
left out:

- `.obsidian/graph.json`, the Obsidian colour settings. tokensave skips hidden
  directories, so it would not be read anyway.
- The exact order of ties in `graphify_dominant_confidence`, which depends on
  NetworkX adjacency order. The values here follow insertion order.

To compare the fixture with real Graphify output, run
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
