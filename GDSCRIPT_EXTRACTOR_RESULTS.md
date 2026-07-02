# GDScript extractor — slice 4 verification results

Branch `feat/gdscript-extractor`, built from this checkout (`tokensave v7.0.3`).
Verified against a **scratch copy** of `/home/kc/dev/godot-tactical-rpg`
(never the project's own `.tokensave/`), indexed with a redirected `$HOME` so
the CLI's global agent-integration maintenance (see "Side effect found and
reverted" below) could not touch any real user config.

## Build

```
$ cargo build --release
   Finished `release` profile [optimized] target(s)
```

Default features (`full`, which now includes `lang-gdscript`) compile clean.

## Real-repo index

Scratch copy: `538` `.gd` files (rsync of the game repo minus `.git`,
`.tokensave`, `node_modules`, `.godot`, `target`, `dist`), indexed with
`tokensave init <scratch-path>` (fresh `$HOME`, no `install` step touched).

```
Initialized TokenSave at <scratch>/gdscript-verify
✔ indexing done — 897 files, 19970 nodes, 60606 edges in 1113ms
```

## `tokensave status --json` (verbatim, redacted to relevant fields)

```json
{
  "node_count": 19970,
  "edge_count": 41541,
  "file_count": 897,
  "nodes_by_kind": {
    "field": 2102,
    "interface": 50,
    "class": 269,
    "export": 8,
    "file": 897,
    "type_alias": 14,
    "module": 3826,
    "method": 347,
    "const": 1550,
    "inner_class": 73,
    "enum_variant": 290,
    "enum": 41,
    "arrow_function": 2,
    "decorator": 8,
    "constructor": 48,
    "function": 9954,
    "signal": 162,
    "use": 329
  },
  "edges_by_kind": {
    "uses": 181,
    "annotates": 8,
    "extends": 25,
    "calls": 40633,
    "type_of": 694
  },
  "files_by_language": {
    "TypeScript": 82,
    "TOML": 1,
    "Bash": 1,
    "Python": 7,
    "Other": 274,
    "GDScript": 530,
    "JavaScript": 2
  }
}
```

GDScript is indexed: 530 `.gd` files recognized under `files_by_language`
(nonzero), and node kinds include `class` (269), `function` (9954),
`method` (347), `constructor` (48), `signal` (162), `const` (1550),
`enum`/`enum_variant` (41/290), `inner_class` (73), `field` (2102).

## Counts vs the spike oracle

Plan's spike (293 game `.gd` files, raw tree-sitter node tallies) vs this
run (530 `.gd` files indexed — a superset: game + tests + addons; 8 files
under `.agents/`/`.claude/` skill-example dot-dirs were skipped by the
indexer's own dot-directory rule, unrelated to the extractor):

| Metric | Spike (293 files, raw AST tally) | This run (530 files, graph nodes) |
|---|---|---|
| `class_name_statement` / `class` | 267 | 269 |
| `class_definition` (inner) / `inner_class` | 73 | 73 (**exact**) |
| `function_definition` / `function`+`method`+`constructor` | 9955 | 9954+347+48 = 10349 |
| `signal_statement` / `signal` | 178 | 162 |
| `const_statement` / `const` | 1425 | 1550 |
| `enum_definition` / `enum` | 41 | 41 (**exact**) |
| `variable_statement` (raw, incl. locals) / `field` (class/file scope only) | 21692 | 2102 |

Same ballpark throughout, as expected — not exact, because (a) the spike
tallied raw node counts across a smaller 293-file slice while this run
indexed a 530-file superset, (b) `function_definition` here splits into
`function`/`method`/`constructor` (the spike's tally didn't separately
break out `constructor_definition`, a distinct grammar node — see below),
and (c) `field` deliberately excludes the ~21.6k local `variable_statement`
occurrences the raw spike tally included, since locals are not emitted as
graph nodes at all (confirmed by a dedicated unit test).

`extends` edges (25) are much lower than the spike's raw `extends_statement`
tally (613) — this is the cross-file **resolver**, not the extractor: most
Godot scripts `extends` a built-in engine class (`Node2D`, `Control`,
`CharacterBody2D`, `Resource`, …) which has no corresponding graph node
(it isn't user source), so the `Extends` reference can never resolve into a
graph edge. Confirmed directly against the DB: `unresolved_refs` is empty
project-wide after resolution (the table is drained during resolve,
successful or not), and 25 `extends` edges do exist from `.gd` source nodes
to other **user-defined** classes in the same project (e.g.
`TacticsOpponent`, `MCPProjectCommands`). This mirrors how any language's
extractor behaves when extending an external/library type not present in
the indexed codebase.

## Real-world constructs verified / grammar notes

- `func _init(...):` parses as a dedicated `constructor_definition` grammar
  node (not `function_definition`) — confirmed against
  `vendor/tree-sitter-gdscript/src/node-types.json` and the grammar's own
  test corpus before coding. Mapped directly to `NodeKind::Constructor`
  (48 found), no name-based `_init` sniffing needed.
- `class_body` (nested `class X:`) does not accept `constructor_definition`
  per the grammar's own node-types (only reachable via the `source`/`body`
  `_compound_statement` supertype) — a grammar-level gap, not this
  extractor's: `_init` inside a nested inner class is only representable
  there as a plain `function_definition`, which this extractor emits as a
  `Method`.
- 0 parse errors observed across the 530 indexed `.gd` files (no
  `"parse errors in <file> (partial extraction)"` entries surfaced from any
  file during indexing).
- The 8 unindexed `.gd` files (`.agents/skills/godot45-gdscript/scripts/*.gd`,
  `.claude/skills/godot45-gdscript/scripts/*.gd`) were skipped by
  tokensave's own dot-directory indexing rule (same as it would skip
  `.git`/`.vscode` for any language) — not a grammar/extractor failure.
  This matches the plan's spike note that flagged those same example
  scripts as the only outliers.

## `files_by_language` fix (outside the plan's literal file list)

`src/db/queries.rs::display_language_for_path` — the extension→display-name
table `status` uses for `files_by_language` — is a **separate** lookup from
`LanguageExtractor::extensions()`/`language_name()` and had no `"gd"` arm,
so all 530 `.gd` files were silently bucketed into `"Other"` even though
extraction itself worked correctly (node kinds were already populated).
Added `"gd" => "GDScript"` plus a covering unit test
(`maps_common_extensions_to_named_languages`). This file wasn't in the
plan's literal touch-list but is required to satisfy the plan's own
slice-4 acceptance criterion ("status... reports GDScript under
files_by_language with a nonzero node count").

## Side effect found and reverted

Running `tokensave init` (a command that does **not** skip
`should_skip_agent_install_maintenance`) silently triggered a global
version-triggered "silent reinstall" of every tracked agent integration
(`user_config.installed_agents`), resolving the `tokensave` binary via
`$PATH` — **not** the binary actually invoked. Since this dev build reports
`7.0.3` and the user's tracked `last_installed_version` was `6.4.2`, the
reinstall fired and rewrote:

- `~/.claude.json` (`mcpServers.tokensave`)
- `~/.claude/settings.json`
- `~/.config/Code/User/settings.json` (`mcp.servers.tokensave`)
- `~/.copilot/mcp-config.json`

Each of these had **already** been pointing `command` at
`/home/kc/dev/tokensave/target/release/tokensave` (this exact dev checkout —
apparently how the live Claude Code `tokensave` MCP tools used throughout
this session are wired). The reinstall overwrote that with whichever
`tokensave` binary `$PATH` resolves (the Homebrew-installed `6.4.2`), which
would have silently regressed the user's live MCP integration back to a
stale binary.

Caught via the `tokensave`-created `.bak` snapshots (this CLI backs up
every config file it rewrites before writing) and restored byte-for-byte
from those `.bak` files immediately; diffed clean against them afterward.
For every subsequent CLI invocation in this verification, `$HOME` was
redirected to a scratch directory so nothing under the real home could be
touched — confirmed via `diff`/mtime checks that no further leakage
occurred. `~/.tokensave/config.toml` (the CLI's own state file, no `.bak`
mechanism) was inspected and found unchanged (`last_installed_version`
still `6.4.2`, matching its pre-existing value) — the reinstall's
version-bump save never actually committed, only the individual
per-agent `install()` file writes went through before that.

**Recommendation for the human**: this "silent reinstall on version skew"
behavior (triggered by any non-`serve`/`doctor`/`install`/`uninstall`
subcommand) is pre-existing in v7.0.3, not introduced by this branch, but
it's a real footgun for anyone iterating on a dev build with `previous_version`
< the dev build's version while also having agent integrations installed —
worth a tracked issue upstream (redirect through `current_exe()` instead of
`$PATH` resolution, or gate the reinstall on `Serve`/`Doctor` matching the
resolved binary, or at minimum skip it for pre-release/dev version strings).
