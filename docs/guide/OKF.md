# Open Knowledge Format (OKF)

The [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
(OKF) is a vendor-neutral format for knowledge: a directory of Markdown files
with YAML frontmatter, called a *bundle*. tokensave can check a bundle against
the OKF conformance rules, export its code index as a bundle, and import
bundles from other tools so that agents can search them through MCP. All `okf`
commands work without a network connection, and none of them changes the code
index.

tokensave implements **OKF v0.2**, checked against specification commit
[`ad30107`](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/ad30107c31c06aec8a7d5636e0d1058118604e6f/SPEC.md).

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

## Importing a bundle

```bash
tokensave okf import ./knowledge                  # Name taken from the directory: "knowledge"
tokensave okf import ./vault --name graphify      # Choose the name
tokensave okf import ./vault --name graphify --replace
tokensave okf import ./knowledge --strict         # Refuse a bundle that is not conformant
tokensave okf list                                # Imported bundles, with counts and source
tokensave okf remove graphify
```

`import` copies the bundle to `okf/<name>/bundle/` in the project's state
directory (`.tokensave/` unless relocated), together with an `import.json` that records
the source path, the time, the declared `okf_version` and the validation
counts. The copy is byte for byte, so the stored bundle is itself an OKF
bundle. The code index and its database are not changed, and no reindex is
needed. The project must already be initialized (`tokensave init`).

The copy is validated after it is made, and `import` prints the result. A
bundle that is not conformant is still imported, because OKF §12 asks
consumers to read what they can. Concept files without a readable `type` are
kept in the copy but cannot be searched. With `--strict`, a bundle that is not
conformant is refused and nothing is stored.

Names use `[A-Za-z0-9_.-]`, start with a letter, digit or `_`, and are at most
64 characters. An existing name is refused unless `--replace` is given. A
bundle is assembled in a temporary directory and moved into place only when
complete. A replaced bundle is moved aside first and restored if the new one
cannot be moved into place.

The reader rules of [Validating a bundle](#validating-a-bundle) apply, with
lower limits, because the MCP server reads imported bundles again: at most
50,000 entries, 4 MiB per file and 64 MiB in total. A bundle over a limit is
refused. Files that are not Markdown, such as attester scripts, are copied
too.

## Using imported knowledge

The `tokensave_knowledge` MCP tool searches and reads every imported bundle of
the project. It is in the `memory` area of `tokensave_more`, so the default
tool list does not change.

| Argument | Effect |
|----------|--------|
| *(none)* | List the imported bundles and how many concepts each can search. |
| `query` | Words to search for in concept titles, IDs, tags, types, descriptions and bodies. Concepts that match more of the words rank first. |
| `file` | Only concepts that refer to this project file. |
| `type` | Only concepts of this `type`, ignoring case. |
| `bundle` | Only this imported bundle. |
| `concept` | Read one concept in full: frontmatter, body (up to 24,000 characters), links with whether their target exists, and backlinks. |
| `related` | Traverse both links and backlinks from one concept. `depth` defaults to 1 and is capped at 3; results include their distance from the starting concept. |
| `limit` | Results to return; default 10, at most 50. |

A concept refers to a project file through one of these frontmatter keys:
`tokensave_path` (written by `tokensave okf export`), `graphify_source_file`
(written by Graphify) or `resource` when it is a relative path. Each result
lists those files under `code_files` with `indexed: true` or `false`, so an
agent can move on to the code tools for files the index knows.

The server reads the imported bundles on first use and keeps them in memory
until one is imported, replaced or removed.

Exporting the code index and importing the result is lossless for what the
export writes: each `Source File` concept maps back to its file through
`tokensave_path`, including paths whose characters the bundle's file names
cannot hold.

## Graphify interoperability

The [Graphify OKF fork](https://github.com/alderac/graphify-okf) writes OKF
bundles as Obsidian vaults (`to_okf_obsidian`). tokensave imports them:

- Graphify adds a `producer: graphify` key to the root `index.md` and
  declares `okf_version: '0.1'`. OKF v0.2 §8 allows only `okf_version` in the
  root index, so `validate` reports `OKF-E004` and an `OKF-I207` note.
  `import` still imports such bundles unless `--strict` is given.
- Each code or document node becomes a concept whose `graphify_source_file`
  links it to the indexed file it came from. Community notes and the
  `## Connections` links are searchable and show up as links and backlinks.
- The hidden `.obsidian/` directory is skipped.

The fork has no OKF reader, so bundles written by `tokensave okf export`
cannot be loaded back into Graphify yet. The test fixture in
`tests/fixtures/okf/graphify/` is output from the pinned Graphify exporter and
is processed by tokensave's validation, import and knowledge-search tests. Its
README records the exact producer commit, environment and reproduction command.

## Compliance matrix

What tokensave does for each part of OKF v0.2. "Validate" is
`tokensave okf validate`; "export" is `tokensave okf export`; "import" is
`tokensave okf import` together with the `tokensave_knowledge` MCP tool.

| Section | Requirement | Validate | Export | Import |
|---------|-------------|----------|--------|--------|
| §3 | Bundle is a directory tree of Markdown files | Reads the tree; skips symbolic links and hidden directories | Writes one | Copies the tree byte for byte, with the same skips |
| §3.1 | `index.md` and `log.md` are reserved | Treated as index and log files | Never used for concepts | Kept, not searched |
| §4 | Concepts are UTF-8 Markdown files with frontmatter | `OKF-E006`, `OKF-E001`, `OKF-E002` | UTF-8 with frontmatter | Unreadable files are kept, not searched |
| §4.1 | `type` is required | `OKF-E003` | `Source File`, `TokenSave Export` | Concepts without one are not searched; `type` filter |
| §4.1 | `title`, `description`, `resource`, `tags` | Shape checked (`OKF-W109`) | `title`, `description`, `tags` | Searched; `tags` as a list or a comma-separated string; a relative `resource` links a project file |
| §4.1 | Unknown keys are kept and never rejected | Never rejected | Extensions use `tokensave_` keys | Kept and returned in full; `tokensave_path` and `graphify_source_file` link project files |
| §4.2 | Conventional headings | `# Computation` checked for Attested Computations | `# Symbols`, `# Dependencies`, `# Dependents` | Body searched and returned |
| §5.1 | `sources`, credibility signals, `usage_window` | Checked (`OKF-W102`, `OKF-W105`, `OKF-W110`) | Not written | Kept in the returned frontmatter |
| §5.1 | Footnotes keyed to `sources[].id` | `OKF-I206` | Not written | Kept in the body |
| §5.2 | `generated` and `verified`; a bare `verified` mapping is a one-element list | Checked (`OKF-W103`, `OKF-W104`) | `generated` on every concept | Kept in the returned frontmatter |
| §5.3 | Trust tiers from `verified` | Not reported | Concepts are unverified | Not ranked by trust |
| §5.4, §5.5 | `status`, `stale_after` | `OKF-W101`; staleness as `OKF-I204` | Not written | `status` returned with each result |
| §6.1 | Absolute and relative links; broken links are tolerated | Resolved; `OKF-I201`, `OKF-I202` | Absolute links | Resolved into links and backlinks, with whether the target exists |
| §6.2 | Path-valued fields | Resolved against the document and the bundle root (`OKF-I201`) | Not written | Kept in the returned frontmatter |
| §7 | Actor convention | Not checked | `tokensave/<version>` | `import.json` records `tokensave/<version>` |
| §8 | Index files: no frontmatter except root `okf_version` | `OKF-E004` | An index in every directory | Imported best-effort; refused with `--strict` |
| §9 | Log files: `YYYY-MM-DD` headings, newest first | `OKF-E005`, `OKF-W113` | Root `log.md` | Kept, not searched |
| §10 | Attested Computation contract | `runtime`, `parameters`, `computation`, `executor`, `attester` (`OKF-W106`, `OKF-W108`, `OKF-W111`) | Not applicable | Kept; attester files copied; never run |
| §11 | Conformance rules 1 to 3 | All three; exit code 1 on failure | Output is conformant | Recorded in `import.json`; `--strict` refuses |
| §12 | `okf_version`; best-effort reading of other versions | Reported; `OKF-I207` | `okf_version: "0.2"` | Recorded; other versions imported best-effort |
| §13 | v0.1 `timestamp` and `# Citations` | `timestamp` noted as `OKF-I205` | Not written | Kept |

The four sample bundles of the specification repository are in
`tests/fixtures/okf/official/` and are validated by the test suite. All four
are conformant. `stackoverflow` gets warnings because its `tags` are
comma-separated strings rather than lists, and info findings for positional
footnotes such as `[^1]`.

## Limitations

- **Imported knowledge is separate from the code graph.** Imported concepts
  are searched by `tokensave_knowledge`, not by `tokensave_search` or
  `tokensave_context`, and they are not graph nodes. They link to code only
  through the file paths described in
  [Using imported knowledge](#using-imported-knowledge).
- **Search is a plain word match.** It ranks by where the words appear and
  has no stemming, synonyms or fuzzy matching. The server keeps the imported
  bundles of the current project in memory, about twice their size on disk.
- **Graphify in one direction only.** tokensave imports a real bundle from the
  [Graphify OKF fork](https://github.com/alderac/graphify-okf), but the fork
  cannot read OKF bundles. Graphify is pinned and run only when regenerating
  the fixture, not as part of tokensave's test suite; see
  [Graphify interoperability](#graphify-interoperability).
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
