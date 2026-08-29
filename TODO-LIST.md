# TODO-LIST

Backlog snapshot: 2026-08-29. Local-only working list, not committed.
`master` is pushed through `1a9b983`.

Owner decisions live on GitHub under the **`PTaL`** label and are not
duplicated here: `gh issue list --repo aovestdipaperino/tokensave --label PTaL`

## Needs your decision (not blocked on anyone else)

- [ ] **#442 (remaining half)** — the partiality signal, the `sync` hint and the
      docs shipped in `5ac7522`. What is left is one decision: should literal
      search scan tracked files the index holds no row for, automatically?
      The opt-in already exists and is now discoverable (`artifact_extensions`
      makes any file literal-searchable), so the case against is that
      auto-scanning reads files the index was configured to leave out. Same
      call covers whether to widen the shipped defaults (`html`, `css`, `txt`)
      — that grows every project's `files` table, and the doc comment says
      the narrowness is deliberate.
      Also open from that issue: whether `".github/**"` alone should re-enable
      descent (include-glob semantics; the hint already names both entries).
      Optional polish: the tally reports every unindexed extension including
      `.png`, deliberately, because the existing `NON_SOURCE_EXTS` list holds
      `txt`/`xml`/`ini`/`conf`. Splitting that list into binary vs text would
      quieten the block — but which extensions count as searchable is a
      judgment call, so it was left alone.
- [ ] **#419 (open part)** — the `✔ Wrote` trigger is fixed and shipped
      (`23eca6d`); what stays open is whether a version-drift refresh should
      exist at all and where (`install`/`reinstall`/`doctor`).
- [ ] **#458 (open part)** — the honesty fix shipped (`5186090`): the tool
      description, the `field` parameter and a new `qualifier_note` all now
      say the `Type::field` form is accepted but not applied. What is left is
      your call between the reporter's two options: implement real narrowing
      (needs receiver type resolution, a feature not a fix) or reject the
      qualified form outright (makes a documented feature error out). Same
      shape of question for `tokensave_constructors` returning a confident
      `match_count: 0` on non-Rust source.
- [x] **#451** — **answered and approved.** Semantics agreed: split on
      `&&`/`||`/`;`, redirect only when *no* segment has side effects, so the
      labelled batch is caught (`echo` is inert) but `git checkout -b x && rg
      Sym src/` is never eaten. Inert list stays short and conservative, with
      unknown commands counting as having side effects; no parsing beyond the
      three top-level operators. Reporter is writing the patch — issue marked
      `in-progress`.
- [ ] **#449** — literal/structural grep patterns (`-c` counts, conflict
      markers, regex with structural parens) classify as symbol-shaped and get
      blocked, with no tokensave tool that could answer them. Fix direction is
      clear but "what counts as structural" is a judgment call worth your eye.
- [ ] **#450 (open part)** — the `doctor` discovery check shipped (PR #471):
      a new `Index scope` section flags `$HOME` initialized as a project (at
      any size, from any cwd) and a project index past 5 GB, WAL included.
      What stays open is the reporter's other two, both of which are yours:
      (1) apply the #396 scope cap to *already-initialized* projects — a real
      behaviour change, since it decides which existing indexes stop working;
      (2) a SIGTERM handler that interrupts an in-flight sync, since today the
      busy sync swallows it and SIGKILL is required. Also unfixed: the server
      surviving parent death (PPID 1), which is defect 3 in the #396 triage.
- [ ] **#452** — `CODE_DIRS` is 8 hardcoded names and Grep/Read have no
      handler at all. Same classifier as #448/#449; needs a decision on
      whether the allow-list should be derived rather than listed.
- [ ] **CI: `Star History` scheduled workflow has failed daily since at least
      2026-08-19** — `401 Access Token Unauthorized` at the render step. Needs
      a token secret created or rotated, which only you can do. Six
      consecutive red scheduled runs are also what makes a real failure easy
      to miss.

## Blocked on someone else

- [ ] **#346** — `pending-verification`. Parts 2(a)/2(c)/2(d) fixed; parts 1
      (fabricated cross-language SCC) and 2(b) (build tags) not reproducible
      across five fixtures. Waiting on two rows of the offending *edges*.
- [ ] **#468** — `install` registering a hard-coded `/usr/local/bin/tokensave`.
      Verified **not reproducible** on current `master`: the reporter is on
      7.0.2 and `preserve_mcp_command_str` (#161, first released v7.1.0)
      replaces a previous command that no longer resolves. Confirmed by
      running `install` from a non-standard prefix over a pre-seeded stale
      entry — it rewrote to the real path. Labelled `pending-verification`,
      awaiting the reporter's re-check on 7.10.0.
- [ ] **PR #427** — C/C++ header extraction. Owner review in flight, author has
      folded in both asks plus three self-found blanking-scanner defects.
      Now conflicts on **`CHANGELOG.md` only** (verified by local merge);
      author notified that a rebase costs one file.
- [ ] **#376** — federate one query across several `graph_root`s; parked
      pending @paulvno confirming whether #375 alone unblocks them.

## Longer-term feature requests (not bugs)

- [ ] #409 Resolver redesign: bound the resolution pass (`PTaL`)
- [ ] #437 / #421 No surface reports which project a bare `serve` serves
- [ ] #436 One `tokensave.exe` per Codex subagent, never exits (stdin EOF
      never arrives under a live supervisor)
- [ ] #342 Making tokensave "just work" (auto index management, follow-up to #179)
- [ ] #309 Companion docs: phase 2/3 and section anchors
- [ ] #306 Resolution path still materialises the whole node graph at once
- [ ] #226 Automatic fallback from local project DB to repo-wide DB
- [ ] #48 Custom parser for compilers/interpreters (P3-low)

## Done this session (2026-08-29) — all pushed

| Item | Commit | Outcome |
|---|---|---|
| **PR #467** branch-drift tool-coverage invariant (#463) | `4f756c7` | **merged** (squash); #463 closed. This is the follow-up flagged at the bottom of the 2026-08-24 list — the six-name runtime list is now derived-and-asserted against the live registry, so a new selector-less tool cannot merge unclassified |
| **PR #453** `post-merge` git hook | `dd8696c` | **squashed locally** — conflicted on `CHANGELOG.md` only; PR closed by hand with authorship preserved. Verified `hooks.rs`/`branch.rs`/`cli.rs` byte-identical to the author's branch before pushing |
| **#457** `field_sites` counts tracked the limit | `5186090` | **closed**. `write_count`/`read_count` are now true totals; added `write_returned`/`read_returned`, `write_lines`/`read_lines`, `truncated`. 3 tests, all verified failing pre-fix |
| **#458** `field_sites` qualifier parsed but not applied | `5186090` | **half fixed** — see decision item above. Tool description, parameter description and a `qualifier_note` now state it is not applied; a test pins that a qualified call returns exactly the bare-name sites |
| **#448** grep hook blocked config-excluded paths | `95073be` | **closed**. Hook now reads the project's own `exclude` globs. 4 tests, two of which are guards that the guardrail still fires for indexed source and for an empty `exclude` |
| **#468** hard-coded `/usr/local/bin` MCP command | — | **not reproducible** on `master`; fixed by #161 in v7.1.0. Reporter on 7.0.2, asked to re-verify |
| **#450** runaway home-directory index | `0ea97ff` | **discovery half done** — `doctor` now surfaces the state; the scope cap and SIGTERM handler stay open (see above) |
| **#459** Obsidian `.canvas` extractor | `6df0950` | **closed**. Hand-rolled `serde_json` reader, no grammar table. Text cards → `Module` nodes with full markdown as docstring; `file` cards → `Uses` edges to the note; `edges[]` → card-to-card edges. `.base` deliberately not covered. 7 tests; verified end-to-end against a real vault. New `lang-canvas` feature in the `full` tier |
| **PR #427** C/C++ headers | — | **reviewed** (second round). Merges cleanly with current `master`, CI green, full suite passes on the merge. One blocking finding: `is_default_member_initializer` misroutes valid plain C (`struct S { enum { KMax = 8 }; ... }`) to `CppExtractor` — confirmed by calling `header_dialect` directly. Two non-blocking notes, one of which was a first-pass finding I retracted on the PR |
| **#450/#436 SIGTERM** | `1a9b983` | **reapability fixed.** Root cause was *not* the busy-sync hypothesis: `tokio::io::stdin()` reads on an uncancellable blocking thread, so the runtime drop waited on it forever under a supervisor holding stdin — the server ran full graceful shutdown, printed its summary, and stayed alive 30s+. Only stdin closing had ever really stopped a server. Serve path now flushes and `process::exit(0)`s after shutdown. Second defect fixed alongside: SIGTERM stream was rebuilt per loop iteration so it was absent during `handle_request`, and tokio never restores the default disposition on drop. `tests/serve_sigterm_test.rs` hangs 20s pre-fix, exits in 0.0s now |
| **Triage pass** | — | `enhancement` applied to #459/#455/#421/#409/#342/#48; `PTaL` applied to the seven design-decision bugs (#458/#452/#450/#449/#442/#436/#419); new `BBRI` label (blocked by reporter input) created and applied to #346 — its reporter @drewjocham has never responded since filing; #451 marked `in-progress`; #468 closed as fixed-in-v7.1.0 |

Thank-you notes posted on #467, #453, #457, #448; status notes on #458, #468 and #450.

## Follow-up worth filing

- ~~`LOCAL_GRAPH_TOOLS_NOT_SUPPORTING_SELECTORS` is a hard-coded list of six
  tool names, and nothing fails when a new selector-less tool is added.~~
  **Done** — @rNoz filed it as #463 and fixed it in PR #467 (`4f756c7`).

- `field_sites` still returns one entry per *occurrence*, so two references on
  one line are two entries with an identical `file`, `line` and `snippet`.
  #457 made the two quantities separately countable (`write_count` vs
  `write_lines`), which removes the misleading-number problem, but the site
  list itself is still un-deduplicated. Collapsing it, or offering a
  `dedupe_lines` option, is a smaller decision now that the counts are honest.
