# Official OKF sample bundles

The sample bundles of the OKF specification, one directory per OKF version.
They are used by `tests/okf_test.rs` as conformance and regression fixtures
(#858, #1060). The generated `viz.html` viewers are not included.

| Directory | OKF version | Source | Commit |
|-----------|-------------|--------|--------|
| `v0.2/` | 0.2 | `bundles/` of [GoogleCloudPlatform/open-knowledge-format](https://github.com/GoogleCloudPlatform/open-knowledge-format) | `ad30107c31c06aec8a7d5636e0d1058118604e6f` |
| `v0.3/` | 0.3 | `bundles/` of [open-knowledge-format/open-knowledge-format](https://github.com/open-knowledge-format/open-knowledge-format) | `527e81aff55992be21507e97004286789126f88c` |

The Markdown files and `acme_retail/attesters/sql_equality.py` are copied
unchanged, except that line endings are LF. The `v0.2/` bundles declare no
`okf_version`, so tokensave reads them with the v0.2 fallbacks of OKF v0.3
§12.1. The `v0.3/` bundles declare `okf_version: "0.3"`.

They are licensed under the Apache License 2.0; see `LICENSE.txt`. Update the
`v0.3/` copy together with `SPEC_COMMIT` in `src/okf/mod.rs`. Keep `v0.2/`
unchanged: it is the regression set for bundles written before OKF v0.3.
