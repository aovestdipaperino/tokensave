# Open Knowledge Format (OKF)

The [Open Knowledge Format](https://github.com/open-knowledge-format/open-knowledge-format)
(OKF) is a vendor-neutral format for knowledge: a directory of Markdown files
with YAML frontmatter, called a *bundle*. tokensave can check a bundle against
the OKF conformance rules, export its code index as a bundle, and import
bundles from other tools so that agents can search them through MCP. All `okf`
commands work without a network connection, and none of them changes the code
index.

tokensave implements **OKF v0.3**, checked against specification commit
[`527e81a`](https://github.com/open-knowledge-format/open-knowledge-format/blob/527e81aff55992be21507e97004286789126f88c/SPEC.md),
and reads bundles of earlier versions with the fallbacks that v0.3 allows.

## Supported versions

A bundle declares its version with `okf_version` in the frontmatter of its
root `index.md` (OKF §8.1). tokensave reads it in one of three modes, shown
as `read_mode` by `validate --json` and by the `tokensave_knowledge` tool.

| Declared `okf_version` | Read mode | Rules |
|------------------------|-----------|-------|
| `"0.3"` | `current` | OKF v0.3. |
| `"0.2"`, `"0.1"` or none | `legacy` | OKF v0.3 with the fallbacks of §12.1 and tokensave's earlier readings, listed below. |
| Any other value | `best_effort` | OKF v0.3, best effort, as §12 asks. |

The `legacy` mode differs from `current` in these points only:

- A path field without a leading `/` that names nothing from the directory of
  its file is tried from the bundle root (§6.2, §12.1). OKF v0.2 examples
  wrote such paths. `validate` notes each use as `OKF-I211`.
- A v0.1 `timestamp` stands in for an absent `generated.at` when trust tiers
  are calculated (Appendix B.1).
- A `log.md` with frontmatter is a warning (`OKF-W116`), not an error. OKF
  v0.2 sample bundles have one.
- A scalar `tags` value is split at commas, as tokensave did before. In
  v0.3 a scalar is one tag (§11.2).

tokensave never rewrites a bundle to convert it to v0.3. To migrate a bundle,
fix the `OKF-I211` and `OKF-W116` findings and then declare
`okf_version: "0.3"`.

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
OKF-E007 [error] log.md:1: log.md has no frontmatter (§9, §11.1 rule 2)
OKF v0.3: NOT conformant — 9 concepts, 7 index and 1 log files, declares okf_version 0.3; 1 errors, 0 warnings, 0 info
```

The JSON report also has `read_mode`, the bundle `metadata` (§8.1) and
`non_concepts`, the number of Markdown files that are not concepts.

| Severity | Meaning |
|----------|---------|
| error | The bundle breaks one of the two conformance rules of OKF §11.1, or could not be fully checked. |
| warning | The bundle deviates from guidance in §3 to §10 that a producer can fix. The bundle is still conformant. |
| info | Something consumers must tolerate, such as a broken link or a stale concept, or a note about how the bundle was read. Hidden unless `--verbose` or `--json` is used. |

| Code | Severity | Finding |
|------|----------|---------|
| `OKF-E000` | error | A size limit stopped the check before the whole bundle was read. |
| `OKF-E002` | error | Frontmatter does not parse as a YAML mapping: no closing `---`, invalid YAML, duplicate keys, not a mapping, or too large. |
| `OKF-E004` | error | An `index.md` outside the bundle root has frontmatter. |
| `OKF-E005` | error | A `## ` date heading in `log.md` is not in `YYYY-MM-DD` form. |
| `OKF-E006` | error | A Markdown file is not valid UTF-8. |
| `OKF-E007` | error | A `log.md` has frontmatter (`current` and `best_effort` modes). |
| `OKF-W101` | warning | `status` is not `draft`, `stable` or `deprecated`. |
| `OKF-W102` | warning | A timestamp is neither an RFC 3339 date-time with an offset nor a date, or a `usage_window` lacks `from` or `to`. |
| `OKF-W103` | warning | `generated` is not a mapping or has no `by`. |
| `OKF-W104` | warning | A `verified` entry lacks `by` or `at`, so consumers ignore it. |
| `OKF-W105` | warning | A `sources` entry lacks `resource`, is not a mapping, or has an invalid `usage_count`. |
| `OKF-W106` | warning | An Attested Computation has no `runtime`. |
| `OKF-W108` | warning | Attested Computation `parameters`, `computation`, `executor` or `attester` are malformed. |
| `OKF-W109` | warning | `title`, `description`, `resource` or `tags` have the wrong shape, or `tags` is one string with commas. |
| `OKF-W110` | warning | Two `sources` entries of one concept share an `id`. |
| `OKF-W111` | warning | An Attested Computation has no computation: no `computation` file in the bundle and no fenced code block in a `Computation` section. |
| `OKF-W112` | warning | `okf_version` is not a quoted string. |
| `OKF-W113` | warning | `log.md` dates are not newest first. |
| `OKF-W114` | warning | Two paths differ only in letter case, so one is lost on case-insensitive file systems. |
| `OKF-W115` | warning | Bundle metadata `title`, `description` or `license` is not a string. |
| `OKF-W116` | warning | A `log.md` has frontmatter (`legacy` mode). |
| `OKF-W117` | warning | A Markdown file has no frontmatter, so it is not a concept. |
| `OKF-W118` | warning | A Markdown file has frontmatter without a non-empty string `type`, so it is not a concept. |
| `OKF-W119` | warning | `generated.by`, `verified[].by` or `sources[].author` is not an actor (`<producer>/<version>` or `<scheme>:<id>`). |
| `OKF-W120` | warning | `superseded_by` is set but `status` is not `deprecated`. |
| `OKF-W121` | warning | `superseded_by` is not one path, or names nothing in the bundle or a file that is not a concept. |
| `OKF-W122` | warning | A `superseded_by` chain leads back to the concept. |
| `OKF-W123` | warning | A path field without a leading `/` resolves only from the bundle root, which OKF v0.3 no longer does (`current` and `best_effort` modes). |
| `OKF-W124` | warning | Frontmatter uses custom YAML tags. |
| `OKF-I201` | info | A link or path field points at nothing in the bundle. |
| `OKF-I202` | info | A link or path field climbs above the bundle root, so it is external. |
| `OKF-I203` | info or warning | An entry was skipped: a symbolic link, hidden file or hidden directory (info), or a file that is too large or unreadable (warning). |
| `OKF-I204` | info | The concept is stale: its `stale_after` instant has passed. |
| `OKF-I205` | info | A v0.1 `timestamp` is used without `generated`. |
| `OKF-I206` | info | A footnote label does not match any `sources[].id`. |
| `OKF-I207` | info | The bundle declares an `okf_version` other than 0.3; the message names the read mode. |
| `OKF-I208` | info | Verifications predate the last change (`generated.at`) and are history only; the message names the trust tier. |
| `OKF-I209` | info | An Attested Computation is not attestable; the message names what is missing. |
| `OKF-I210` | info | The bundle declares no `okf_version`, so it is read in `legacy` mode. |
| `OKF-I211` | info | A path field was resolved with the v0.2 bundle-root fallback (`legacy` mode). |

`OKF-E001` and `OKF-E003` are no longer reported. In OKF v0.3 a Markdown file
without frontmatter or without `type` is not a concept and does not make a
bundle non-conformant (§3.2), so these cases are now `OKF-W117` and
`OKF-W118`.

The reader never follows symbolic links and skips files and directories
whose name starts with `.`, such as `.git` (§3.2). It reads at most 100,000
entries, 4 MiB per Markdown file and 512 MiB in total. Frontmatter is limited
to 256 KiB, 64 levels of nesting and 20,000 nodes after YAML aliases are
expanded.

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
  index.md                 # okf_version: "0.3", title, description; the top-level listing
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
TokenSave-specific frontmatter keys are extension keys in the `x-tokensave-`
namespace that OKF §4.1 recommends, for example `x-tokensave-path` (the
original repository path) and `x-tokensave-content-hash`. OKF consumers keep
or ignore such keys.

Path segments are rewritten to `[A-Za-z0-9_.-]`, the concept-ID rule of the
OKF reference implementation. Names that clash with reserved names (`index`,
`log`, Windows device names) or that differ only in letter case get a suffix,
so the bundle checks out intact on every file system.

| Option | Default | Effect |
|--------|---------|--------|
| `--max-files` | 20000 | Refuse indexes with more source files. |
| `--max-symbols` | 500 | Symbols listed per file. The rest are counted, and the concept gets `x-tokensave-symbols-truncated: true`. |
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
bundle, and every frontmatter key, link, provenance field and extension is
kept as written. The code index and its database are not changed, and no
reindex is needed. The project must already be initialized (`tokensave init`).

The copy is validated after it is made, and `import` prints the result. A
bundle that is not conformant is still imported, because OKF §12 asks
consumers to read what they can. Markdown files without frontmatter `type`,
such as a `README.md`, are not concepts (§3.2): they are kept in the copy but
cannot be searched. With `--strict`, a bundle that is not conformant is
refused and nothing is stored.

Names use `[A-Za-z0-9_.-]`, start with a letter, digit or `_`, and are at most
64 characters. An existing name is refused unless `--replace` is given. A
bundle is assembled in a temporary directory and moved into place only when
complete. A replaced bundle is moved aside first and restored if the new one
cannot be moved into place.

The reader rules of [Validating a bundle](#validating-a-bundle) apply, with
lower limits, because the MCP server reads imported bundles again: at most
50,000 entries, 4 MiB per file and 64 MiB in total. A bundle over a limit is
refused. Files that are not Markdown, such as attester scripts, are copied
too. Hidden files and directories are not part of the bundle and are not
copied.

## Using imported knowledge

The `tokensave_knowledge` MCP tool searches and reads every imported bundle of
the project. It is in the `memory` area of `tokensave_more`, so the default
tool list does not change.

| Argument | Effect |
|----------|--------|
| *(none)* | List the imported bundles with their `title`, `description`, `license`, declared version, read mode and how many concepts each can search. |
| `query` | Words to search for in concept titles, IDs, tags, types, descriptions and bodies. Concepts that match more of the words rank first. |
| `file` | Only concepts that refer to this project file. |
| `type` | Only concepts of this `type`, ignoring case. |
| `bundle` | Only this imported bundle. |
| `current_only` | Only current knowledge: concepts whose `status` is `stable`. A deprecated match is replaced by the concept its `superseded_by` chain leads to, and the result names it in `replaces` (§5.4). |
| `concept` | Read one concept in full: frontmatter, body (up to 24,000 characters), links with whether their target exists, path-field links, backlinks, trust and attestation. |
| `related` | Traverse both links and backlinks from one concept. `depth` defaults to 1 and is capped at 3; results include their distance from the starting concept. |
| `limit` | Results to return; default 10, at most 50. |

Each search result and each concept read carries these signals:

| Field | Meaning |
|-------|---------|
| `status`, `current` | The lifecycle `status`, `stable` when absent. Only `stable` is current; `draft`, `deprecated` and unknown values are not (§5.4). |
| `superseded_by`, `replacement` | For a concept with `superseded_by`: the value as written, and the current concept at the end of the chain, or `null` when the chain is broken, external or a cycle. |
| `trust_tier` | `unverified`, `machine-confirmed` or `human-reviewed`, from the current verifications only (§5.3). |
| `freshness` | `fresh`, `stale`, `malformed` (a `stale_after` that cannot be read is never fresh) or `unknown` (no `stale_after`), at the time of the call (§5.5). |
| `attestable` | Attested Computations only: whether the concept has a `runtime`, an `attester` file and a computation (§10.2, §10.3). |

A concept read adds `trust` with the `current` and `earlier` verifications and
the number of `ignored` incomplete ones, `attestation_gaps` for an Attested
Computation that is not attestable, and `field_links`: the path fields that
resolve to a concept, such as `sources[0].resource`.

Trust follows OKF §5.3: a verification is current when its `at` is equal to
or later than `generated.at`, or when there is no `generated`. Earlier
verifications stay as history and never count, so a concept that an agent
rewrote after a human review is not `human-reviewed` until a person confirms
it again. Only `human:<id>` actors give the `human-reviewed` tier; `team:`
and other schemes count as machine confirmation. Where the specification is
silent, tokensave follows the OKF reference implementation: a `generated`
without `at` keeps every verification current, and a `generated.at` that
cannot be read makes none current.

The concept graph has one directed, untyped edge for every Markdown link to a
concept and for every path field (`resource`, `sources[].resource`,
`computation`, `executor.resource`, `attester.resource`, `superseded_by`) that
resolves to a concept (§6.4). tokensave adds no typed or hierarchical
meaning to these edges. Links to assets, directories and files outside the
bundle are not concept links. A link to a missing `.md` file is kept with
`exists: false`, because it may be knowledge that nobody has written yet
(§6.1).

A concept refers to a project file through one of these frontmatter keys:
`x-tokensave-path` (written by `tokensave okf export`), `tokensave_path`
(written by exports before OKF v0.3 support), `graphify_source_file` (written
by Graphify) or `resource` when it is a relative path that names nothing in
the bundle. Each result lists those files under `code_files` with
`indexed: true` or `false`, so an agent can move on to the code tools for
files the index knows.

The server reads the imported bundles on first use and keeps them in memory
until one is imported, replaced or removed.

Exporting the code index and importing the result is lossless for what the
export writes: each `Source File` concept maps back to its file through
`x-tokensave-path`, including paths whose characters the bundle's file names
cannot hold.

## Graphify interoperability

The [Graphify OKF fork](https://github.com/alderac/graphify-okf) writes OKF
bundles as Obsidian vaults (`to_okf_obsidian`). tokensave imports them:

- Graphify adds a `producer: graphify` key to the root `index.md` and
  declares `okf_version: '0.1'`. OKF v0.3 permits extension keys in the
  bundle metadata (§8.1), so the bundle is conformant and imports with
  `--strict`. It is read in `legacy` mode, and `validate` reports an
  `OKF-I207` note.
- Each code or document node becomes a concept whose `graphify_source_file`
  links it to the indexed file it came from. Community notes and the
  `## Connections` links are searchable and show up as links and backlinks.
- The hidden `.obsidian/` directory and `.graphify_okf_manifest.json` file
  are not part of the bundle and are not copied.

The fork has no OKF reader, so bundles written by `tokensave okf export`
cannot be loaded back into Graphify yet. The test fixture in
`tests/fixtures/okf/graphify/` is output from the pinned Graphify exporter and
is processed by tokensave's validation, import and knowledge-search tests. Its
README records the exact producer commit, environment and reproduction command.

## Compliance matrix

### Consumer requirements of OKF v0.3

Every MUST of OKF v0.3 that applies to a consumer, and how tokensave meets it.
The tests are in `tests/okf_test.rs` and the unit tests of `src/okf/`.

| Section | Requirement | How tokensave meets it |
|---------|-------------|------------------------|
| §3.1, §11.2 | Do not treat `index.md` or `log.md` as concepts, whatever frontmatter they contain | Reserved by name at every level; never searched or counted as concepts |
| §3.2 | Files and directories whose names start with `.` are not part of the bundle; do not follow symbolic links out of the bundle | Skipped and not copied on import (`OKF-I203`); symbolic links are never followed |
| §4.1, §11.2 | Accept unknown `type` values; do not reject unknown keys at any level; a producer that modifies a file keeps unknown keys | Never rejected; imports are byte-for-byte copies, so nested and namespaced extension keys survive and are returned in full |
| §5 | Accept a quoted string and a YAML-native timestamp; a date means the start of that day in UTC; no other form is valid | Both forms are read as text and parsed as an RFC 3339 date-time with an offset or a date |
| §5.2 | Treat a bare `verified` mapping as a list with one item | `validate` and the trust tiers |
| §5.3, §11.2 | Calculate trust tiers only from current verification events | `trust_tier`; earlier events are kept as history in `trust.earlier` |
| §5.5, §11.2 | Do not treat a `stale_after` value that cannot be parsed as fresh | `freshness: malformed`; `OKF-W102` |
| §6, §11.2 | Resolve both in-bundle forms of links and path fields; do not read a value outside the bundle root from the file system | Root (`/x`) and file-relative forms are resolved for links, path fields and graph edges; values that climb out are external and only reported (`OKF-I202`) |
| §6.1 | Accept broken links | Reported as info only (`OKF-I201`); kept with `exists: false` |
| §7 | Accept other actor schemes and treat them as non-human | Only `human:<id>` gives `human-reviewed` |
| §10.2, §11.2 | Do not show a value as attested without `runtime` and `attester` | `attestable` is false without them, and also without an attester file or a computation; tokensave never runs anything |
| §10.3 | A computation whose file does not exist or whose section has no fenced block is missing; treat the concept as not attestable | `attestation_gaps`; `OKF-W111`, `OKF-I209` |
| §11.2 | Do not reject a concept because an optional family is missing | Missing families are never errors |
| §11.2 | Accept one item as a list with one item for `tags`, `sources`, `verified`, `parameters`; do not split a scalar `tags` value at commas | Single items are accepted everywhere; a scalar tag is one tag (except in `legacy` mode, as before) |
| §11.2 | Do not reject a bundle for missing optional fields, unknown types, unknown extension keys, broken links or missing `index.md` | None of these is an error |

### Feature matrix

What tokensave does for each part of OKF v0.3. "Validate" is
`tokensave okf validate`; "export" is `tokensave okf export`; "import" is
`tokensave okf import` together with the `tokensave_knowledge` MCP tool.

| Section | Topic | Validate | Export | Import |
|---------|-------|----------|--------|--------|
| §3 | Bundle structure, assets | Reads the tree; skips symbolic links and hidden entries | Writes one | Copies the tree byte for byte, assets included |
| §3.2 | Concepts are `.md` files with a non-empty `type` | Others get `OKF-W117`, `OKF-W118`; `non_concepts` count | Every concept has a `type` | Others are kept, not searched |
| §4 | Frontmatter syntax: YAML 1.2 mapping, no duplicate keys, no custom tags | `OKF-E002`, `OKF-W124` | JSON flow values, a subset of YAML 1.2 | Files whose frontmatter does not parse are kept, not searched |
| §4.1 | `title`, `description`, `resource`, `tags`; extension keys | Shapes checked (`OKF-W109`) | `x-tokensave-` keys | Searched; extension keys returned in full |
| §4.2 | Body: CommonMark, GFM tables, footnotes | Links, headings, fences and footnotes scanned | Tables and lists | Body searched and returned |
| §5.1 | `sources`, credibility signals, `usage_window`, footnote attribution | `OKF-W102`, `OKF-W105`, `OKF-W110`, `OKF-W119`, `OKF-I206` | Not written | Kept; in-bundle sources are graph edges |
| §5.2, §5.3 | `generated`, `verified`, trust tiers | `OKF-W103`, `OKF-W104`, `OKF-W119`, `OKF-I208` | `generated` on every concept | `trust_tier`, `trust` |
| §5.4 | `status`, `superseded_by` | `OKF-W101`, `OKF-W120` to `OKF-W122` | Not written | `status`, `current`, `replacement`, `current_only` |
| §5.5 | `stale_after` | `OKF-W102`, `OKF-I204` | Not written | `freshness` |
| §6 | Link and path resolution | `OKF-I201`, `OKF-I202`, `OKF-W123`, `OKF-I211` | Root-relative links | Resolved for links, backlinks and `related` |
| §6.4 | Bundle graph: links and path fields | Both resolved | Links between source files | `links`, `field_links`, `backlinks`, `related` |
| §7 | Actor convention | `OKF-W119` | `tokensave/<version>` | `human:` gives `human-reviewed` |
| §8, §8.1 | Index files; bundle metadata | `OKF-E004`, `OKF-W112`, `OKF-W115`; `metadata` in the report | `okf_version`, `title`, `description` in the root `index.md` | Listed with each bundle |
| §9 | Log files: no frontmatter, `YYYY-MM-DD` headings, newest first | `OKF-E005`, `OKF-E007` (`OKF-W116` in `legacy` mode), `OKF-W113` | Root `log.md` | Kept, not searched |
| §10 | Attested Computation contract | `OKF-W106`, `OKF-W108`, `OKF-W111`, `OKF-I209` | Not applicable | `attestable`, `attestation_gaps`; attesters copied, never run |
| §11.1 | Conformance rules 1 and 2 | Exit code 1 on failure | Output is conformant | Recorded in `import.json`; `--strict` refuses |
| §12 | `okf_version`, best-effort reading, v0.2 fallbacks | `read_mode`; `OKF-I207`, `OKF-I210` | `okf_version: "0.3"` | Each bundle read in its own mode |
| Appendix B | v0.1 `timestamp`, `# Citations` | `OKF-I205` | Not written | `timestamp` used for trust in `legacy` mode; `# Citations` kept in the body |

The four sample bundles of the specification repository are vendored for both
versions in `tests/fixtures/okf/official/`: `v0.3/` at the commit above and
`v0.2/` at the commit of the previous support. The v0.3 bundles validate
without any finding. The v0.2 bundles are conformant in `legacy` mode:
`acme_retail` gets an `OKF-W116` warning for its `log.md` frontmatter and
`OKF-I211` notes for its bundle-root path fields, and `stackoverflow` gets
warnings for comma-separated `tags` and info findings for positional
footnotes such as `[^1]`. The same v0.2 bundle relabelled as `0.3` without
migration is tested to fail, as OKF v0.3 §13.1 describes.

## Changes from the OKF v0.2 support

These behaviors changed when tokensave moved to OKF v0.3:

- **Validation.** A Markdown file without frontmatter or `type` is a warning,
  not an error. The bundle-root `index.md` may carry `title`, `description`,
  `license` and extension keys. A `log.md` with frontmatter is an error in
  v0.3 bundles. Date-only timestamps are valid, and offsets must be `Z` or
  `±HH:MM`. New checks cover actors, `superseded_by`, custom YAML tags and
  bundle-root paths in v0.3 bundles.
- **Reading.** Hidden files are no longer part of a bundle and are not
  imported. Path fields without a leading `/` resolve from the file's
  directory, with the bundle-root fallback only in `legacy` mode. A scalar
  `tags` value is one tag in v0.3 bundles.
- **Export.** The root `index.md` declares `okf_version: "0.3"` with a
  `title` and `description`, and TokenSave keys use the `x-tokensave-`
  namespace instead of `tokensave_`. Bundles written by earlier exports still
  map to project files through `tokensave_path`.
- **Knowledge tool.** Results add `current`, `trust_tier`, `freshness`,
  `superseded_by`, `replacement` and `attestable`, and the new `current_only`
  argument filters to current knowledge. Path fields that resolve to a
  concept are graph edges.

## Limitations

- **Imported knowledge is separate from the code graph.** Imported concepts
  are searched by `tokensave_knowledge`, not by `tokensave_search` or
  `tokensave_context`, and they are not graph nodes. They link to code only
  through the file paths described in
  [Using imported knowledge](#using-imported-knowledge).
- **Search is a plain word match.** It ranks by where the words appear and
  has no stemming, synonyms or fuzzy matching. Trust tiers and freshness are
  reported, not used for ranking. The server keeps the imported bundles of
  the current project in memory, about twice their size on disk.
- **tokensave does not edit bundles.** `export` writes a new bundle and never
  regenerates an existing `index.md`, so the §8.1 rule that a producer keeps
  the existing root frontmatter does not arise. `import` stores a
  byte-for-byte copy.
- **Attestation is not run.** tokensave reports whether a computation is
  attestable but never runs executors or attesters, and has no receipts.
- **Credibility signals are not interpreted.** `author`, `usage_count`,
  `last_modified` and `usage_window` are checked and returned, but no
  credibility is inferred from them, and `sources` lineage is not followed
  recursively.
- **Graphify in one direction only.** tokensave imports a real bundle from the
  [Graphify OKF fork](https://github.com/alderac/graphify-okf), but the fork
  cannot read OKF bundles. Graphify is pinned and run only when regenerating
  the fixture, not as part of tokensave's test suite; see
  [Graphify interoperability](#graphify-interoperability).
- **Markdown scanning is minimal.** Links, headings, fences and footnotes are
  found by a small scanner covering the CommonMark forms OKF bundles use, not
  a full CommonMark parser. Code blocks and code spans are skipped. HTML links
  are not followed.
- **Native YAML timestamps are read as text.** The YAML loader keeps
  unquoted timestamps as strings, which are then parsed like quoted ones, so
  both forms behave the same.

## Version policy

The validator checks against OKF v0.3. A bundle that declares another
`okf_version` is still checked, as §12 asks, and gets an `OKF-I207` note that
names its read mode. Support for a new OKF version is added as a separate
change that updates the compliance matrix, the pinned specification commit
and a new versioned directory of sample bundles, keeping the earlier ones as
regression fixtures.
