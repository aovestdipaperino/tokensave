# Open Knowledge Format (OKF)

The [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
(OKF) is a vendor-neutral format for knowledge: a directory of Markdown files
with YAML frontmatter, called a *bundle*. tokensave can check a bundle against
the OKF conformance rules and export its code index as a bundle. Both commands
work without a network connection and do not change the index.

tokensave implements **OKF v0.2**, checked against specification commit
[`ad30107`](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/ad30107c31c06aec8a7d5636e0d1058118604e6f/SPEC.md).
Importing bundles into the index is not available yet; see
[Limitations](#limitations).

## Validating a bundle

```bash
tokensave okf validate ./knowledge             # Errors and warnings, then a summary
tokensave okf validate ./knowledge --verbose   # Also list info findings (broken links, staleness)
tokensave okf validate ./knowledge --json      # Full report as JSON
tokensave okf validate ./knowledge --strict    # Fail on warnings too
```

The command exits with 0 when the bundle is conformant and 1 when it is not.
With `--strict`, warnings also give exit code 1. It reads only the bundle
directory and needs no tokensave project.

Each finding has a stable code, a severity, a path and usually a line:

```text
OKF-E003 [error] metrics/revenue.md:1: frontmatter has no non-empty `type` (§11 rule 2)
OKF v0.2: NOT conformant — 9 concepts, 7 index and 1 log files; 1 errors, 0 warnings, 2 info
```

| Severity | Meaning |
|----------|---------|
| error | The bundle breaks one of the three conformance rules in OKF §11, or could not be fully checked. |
| warning | The bundle deviates from SHOULD-level guidance in §4 to §10 that a producer can fix. The bundle is still conformant. |
| info | Something consumers must tolerate, such as a broken link or a stale concept. Hidden unless `--verbose` or `--json` is used. |

| Code | Severity | Finding |
|------|----------|---------|
| `OKF-E000` | error | A size limit stopped the check before the whole bundle was read. |
| `OKF-E001` | error | A concept file has no frontmatter block. |
| `OKF-E002` | error | Frontmatter is not parseable: no closing `---`, invalid YAML, not a mapping, or too large. |
| `OKF-E003` | error | `type` is missing, empty or not a string. |
| `OKF-E004` | error | An `index.md` has frontmatter outside the bundle root, or the root `index.md` has keys other than `okf_version`. |
| `OKF-E005` | error | A `## ` date heading in `log.md` is not in `YYYY-MM-DD` form. |
| `OKF-E006` | error | A Markdown file is not valid UTF-8. |
| `OKF-W101` | warning | `status` is not `draft`, `stable` or `deprecated`. |
| `OKF-W102` | warning | A timestamp is not an ISO 8601 datetime with a UTC offset, or a `usage_window` lacks `from` or `to`. |
| `OKF-W103` | warning | `generated` is not a mapping or has no `by`. |
| `OKF-W104` | warning | A `verified` entry lacks `by` or `at`. |
| `OKF-W105` | warning | A `sources` entry lacks `resource`, is not a mapping, or has an invalid `usage_count`. |
| `OKF-W106` | warning | An Attested Computation has no `runtime`. |
| `OKF-W108` | warning | Attested Computation `parameters`, `executor` or `attester` are malformed. |
| `OKF-W109` | warning | `title`, `description`, `resource` or `tags` have the wrong shape. |
| `OKF-W110` | warning | Two `sources` entries of one concept share an `id`. |
| `OKF-W111` | warning | An Attested Computation has neither `computation` nor a `# Computation` section. |
| `OKF-W112` | warning | `okf_version` is not a string. |
| `OKF-W113` | warning | `log.md` dates are not newest first. |
| `OKF-W114` | warning | Two paths differ only in letter case, so one is lost on case-insensitive file systems. |
| `OKF-I201` | info | A link or path-valued field points at nothing in the bundle. |
| `OKF-I202` | info | A link climbs above the bundle root. |
| `OKF-I203` | info or warning | An entry was skipped: a symbolic link or hidden directory (info), or a file that is too large or unreadable (warning). |
| `OKF-I204` | info | The concept is stale: its `stale_after` instant has passed. |
| `OKF-I205` | info | A v0.1 `timestamp` is used without `generated`. |
| `OKF-I206` | info | A footnote label does not match any `sources[].id`. |
| `OKF-I207` | info | The bundle declares an `okf_version` other than 0.2; it was checked on a best-effort basis. |

The reader never follows symbolic links and skips directories whose name
starts with `.`, such as `.git`. It reads at most 100,000 entries, 4 MiB per
Markdown file and 512 MiB in total. Frontmatter is limited to 256 KiB, 64
levels of nesting and 20,000 nodes after YAML aliases are expanded.

## Exporting the code index

```bash
tokensave okf export --output ./knowledge
tokensave okf export --project ../other-repo --output ./other-knowledge
```

The export reads the index of the current branch read-only. It does not sync,
so run `tokensave sync` first if the index may be stale. An index whose schema
is older than the running tokensave is refused. The output directory must not
exist; the bundle is assembled in a temporary directory beside it and moved
into place only when complete.

The bundle has this layout:

```text
knowledge/
  index.md                 # okf_version: "0.2" and the top-level listing
  log.md                   # one entry for this export
  tokensave-export.md      # type: TokenSave Export; branch, commit, counts, scope
  files/
    index.md
    src/
      index.md
      main.rs.md           # type: Source File
```

Each indexed source file becomes one `Source File` concept under `files/`,
following the repository layout. Its body has these sections:

- **Symbols**: a table of the file's symbols with kind, name, qualified name,
  lines and visibility.
- **Dependencies**: links to the files it references.
- **Dependents**: links to the files that reference it.

Both link lists give the edge kinds and counts, such as `calls ×3`. Every
concept carries `generated: { by: tokensave/<version>, at: <time> }`.
TokenSave-specific frontmatter keys start with `tokensave_`, for example
`tokensave_path` (the original repository path) and `tokensave_content_hash`.
OKF consumers keep or ignore such keys (§4.1).

Path segments are rewritten to `[A-Za-z0-9_.-]`, the concept-ID rule of the
OKF reference implementation. Names that clash with reserved names (`index`,
`log`, Windows device names) or that differ only in letter case get a suffix,
so the bundle checks out intact on every file system.

| Option | Default | Effect |
|--------|---------|--------|
| `--max-files` | 20000 | Refuse indexes with more source files. |
| `--max-symbols` | 500 | Symbols listed per file. The rest are counted, and the concept gets `tokensave_symbols_truncated: true`. |
| `--max-links` | 200000 | Refuse indexes with more file-to-file dependency rows. |

The export leaves out source code, signatures, docstrings, metrics and
unresolved references. A bundle written by `tokensave okf export` passes
`tokensave okf validate` without findings.

## Compliance matrix

What tokensave does for each part of OKF v0.2. "Validate" is
`tokensave okf validate`; "export" is `tokensave okf export`.

| Section | Requirement | Validate | Export |
|---------|-------------|----------|--------|
| §3 | Bundle is a directory tree of Markdown files | Reads the tree; skips symbolic links and hidden directories | Writes one |
| §3.1 | `index.md` and `log.md` are reserved | Treated as index and log files | Never used for concepts |
| §4 | Concepts are UTF-8 Markdown files with frontmatter | `OKF-E006`, `OKF-E001`, `OKF-E002` | UTF-8 with frontmatter |
| §4.1 | `type` is required | `OKF-E003` | `Source File`, `TokenSave Export` |
| §4.1 | `title`, `description`, `resource`, `tags` | Shape checked (`OKF-W109`) | `title`, `description`, `tags` |
| §4.1 | Unknown keys are kept and never rejected | Never rejected | Extensions use `tokensave_` keys |
| §4.2 | Conventional headings | `# Computation` checked for Attested Computations | `# Symbols`, `# Dependencies`, `# Dependents` |
| §5.1 | `sources`, credibility signals, `usage_window` | Checked (`OKF-W102`, `OKF-W105`, `OKF-W110`) | Not written |
| §5.1 | Footnotes keyed to `sources[].id` | `OKF-I206` | Not written |
| §5.2 | `generated` and `verified`; a bare `verified` mapping is a one-element list | Checked (`OKF-W103`, `OKF-W104`) | `generated` on every concept |
| §5.3 | Trust tiers from `verified` | Not reported | Concepts are unverified |
| §5.4, §5.5 | `status`, `stale_after` | `OKF-W101`; staleness as `OKF-I204` | Not written |
| §6.1 | Absolute and relative links; broken links are tolerated | Resolved; `OKF-I201`, `OKF-I202` | Absolute links |
| §6.2 | Path-valued fields | Resolved against the document and the bundle root (`OKF-I201`) | Not written |
| §7 | Actor convention | Not checked | `tokensave/<version>` |
| §8 | Index files: no frontmatter except root `okf_version` | `OKF-E004` | An index in every directory |
| §9 | Log files: `YYYY-MM-DD` headings, newest first | `OKF-E005`, `OKF-W113` | Root `log.md` |
| §10 | Attested Computation contract | `runtime`, `parameters`, `computation`, `executor`, `attester` (`OKF-W106`, `OKF-W108`, `OKF-W111`) | Not applicable |
| §11 | Conformance rules 1 to 3 | All three; exit code 1 on failure | Output is conformant |
| §12 | `okf_version`; best-effort reading of other versions | Reported; `OKF-I207` | `okf_version: "0.2"` |
| §13 | v0.1 `timestamp` and `# Citations` | `timestamp` noted as `OKF-I205` | Not written |

The four sample bundles of the specification repository are in
`tests/fixtures/okf/official/` and are validated by the test suite. All four
are conformant. `stackoverflow` gets warnings because its `tags` are
comma-separated strings rather than lists, and info findings for positional
footnotes such as `[^1]`.

## Limitations

- **No import yet.** Bundles cannot yet be loaded into the index, searched or
  queried through MCP tools. Import and MCP access are planned as follow-up
  work on [#858](https://github.com/aovestdipaperino/tokensave/issues/858).
- **No Graphify round trip yet.** Bundles written by the
  [Graphify OKF fork](https://github.com/alderac/graphify-okf) can be
  validated, but no automated test runs Graphify itself.
- **Markdown scanning is minimal.** Links, headings and footnotes are found by
  a small scanner covering the CommonMark forms OKF bundles use, not a full
  CommonMark parser. Code blocks and code spans are skipped. HTML links are
  not followed.
- **Path-valued fields are resolved two ways.** OKF §6.2 allows relative
  paths, but the sample bundles write paths such as `policies/x.md` relative
  to the bundle root. A path is reported only when neither reading finds a
  file.

## Version policy

The validator checks against OKF v0.2. A bundle that declares another
`okf_version` is still checked, as §12 asks, and gets an `OKF-I207` note.
Support for a new OKF version is added as a separate change that updates the
compliance matrix, the pinned specification commit and the sample bundles.
