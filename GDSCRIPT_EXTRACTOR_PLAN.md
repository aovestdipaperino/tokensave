# Plan: Add a GDScript extractor to tokensave

**Goal:** tokensave indexes `.gd` (GDScript / Godot 4.x) files into its code graph — classes, functions/methods, signals, consts, fields, enums, `extends`, and call edges — the same way it already handles ActionScript, Ruby, etc. Deliver a locally-built `tokensave` binary that, run on a GDScript project, makes `tokensave_status` report GDScript nodes (not `Other`/0).

**Repo:** this checkout (`~/dev/tokensave`), forked from `aovestdipaperino/tokensave` @ v7.0.3 (`b915f5e`). Work on branch `feat/gdscript-extractor`. Commit each green slice.

**Why this is low-risk:** a spike already proved the hard unknowns (see "Spike evidence" below). The remaining work is mechanical: mirror an existing extractor.

---

## Spike evidence (already proven — do not re-litigate)

Built `tree-sitter-gdscript` (PrestonKnopp, `LANGUAGE_VERSION 14`, has external `scanner.c`) against `tree-sitter = 0.26` with `cc` + gcc-16, parsed all 293 game `.gd` files of a real Godot project:
- **0 parse errors in game code** (4 errors total, all in unrelated skill *template* example scripts — grammar handles real code cleanly).
- Node kinds produced (whole-codebase tallies, the mapping oracle):
  `class_name_statement` 267, `extends_statement` 613, `class_definition` (inner) 73, `function_definition` 9955, `signal_statement` 178, `variable_statement` 21692, `const_statement` 1425, `enum_definition` 41, `call` 40921, `attribute_call` 54562.

Grammar ABI 14 loads fine on tree-sitter 0.26. Toolchain (rustc 1.96.1, gcc-16) is installed and working.

---

## Environment (MANDATORY — every cargo/build command)

```bash
source "$HOME/.cargo/env"
export CC=gcc-16 CXX=g++-16
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc-16
```
There is **no `cc`/`gcc` on PATH** — only `gcc-16` (Homebrew). Without the three exports the build fails with `linker \`cc\` not found`. Prefix shell commands with `rtk`.

---

## Template files to mirror (READ THESE FIRST)

Because GDScript has an **external scanner** (`scanner.c`), split the templates:
- **build.rs vendored-grammar-with-scanner block:** `build.rs` lines ~13–22 (the **WGSL** block — it compiles `parser.c` + `scanner.c`). The ActionScript block (~25–35) is parser-only; do NOT copy that one for the compile step.
- **Extractor implementation:** `src/extraction/actionscript_extractor.rs` (full). It is the closest structural analog: scope stack, `class`/`extends`/`method`/`field`/`const` emission, `Contains`/`Extends`/`Calls` edges, docstring + call-site extraction, graceful `errors` vec (no panic on malformed input). Ruby (`ruby_extractor.rs`) is a secondary reference for a dynamically-typed, block-scoped language.
- **Trait + registration:** `src/extraction/mod.rs` — `LanguageExtractor` trait (~205–220), and the three registration points (mod decl ~35–42, `pub use` ~134–141, `extractors.push(...)` in the "Full" section ~257–273).
- **Types:** `src/types.rs` — `NodeKind` (7–), `EdgeKind` (237–), `Node` (320–), `Edge` (374–), `UnresolvedRef` (394–), `ExtractionResult` (405–).
- **Complexity:** `src/extraction/complexity.rs` — `ACTIONSCRIPT_COMPLEXITY` (~1659). Add a `GDSCRIPT_COMPLEXITY` (GDScript is Python-like: `if`/`elif`/`for`/`while`/`match`/`and`/`or`).
- **Per-language test:** find an existing `tests/*_extraction_test.rs` (e.g. dart/ruby) and mirror its shape.

---

## Node/edge mapping (GDScript tree-sitter kind → tokensave)

Confirm exact node/field names against the grammar's `vendor/tree-sitter-gdscript/src/node-types.json` before coding — names below are from the spike and should be verified.

| GDScript node | tokensave NodeKind | Notes |
|---|---|---|
| file root (`source`) with a `class_name_statement` | `Class` (fallback `Module` if no `class_name`) | file-level script = the class; qualified name from `class_name` or file stem |
| `class_definition` (inner `class X:`) | `InnerClass` | push Class scope |
| `function_definition` at file scope | `Function` | |
| `function_definition` inside a class/inner-class scope | `Method` | constructor = `_init` |
| `function_definition` with `static` | keep `Function`/`Method`; record static in `Visibility`/flags per how actionscript marks statics | |
| `signal_statement` | **new `Signal` variant** (see below) | name + optional typed params |
| `variable_statement` at class/file scope | `Field` | **skip locals**: only emit when the parent scope is File/Class, not inside a `function_definition` body |
| `const_statement` | `Const` | |
| `enum_definition` (+ enumerators) | `Enum` + `EnumVariant` | named or anonymous enums |
| `extends_statement` | `Extends` **edge** | target is an unresolved ref (class name or path string) |
| `call` / `attribute_call` inside a function | `Calls` edge + `UnresolvedRef` | mirror actionscript `extract_call_sites` |
| `Contains` edges | file→class→method/field, per scope stack | |

**Signal kind:** `NodeKind` has no `Signal`. Add one **feature-gated** exactly like the protobuf variants:
```rust
#[cfg(feature = "lang-gdscript")]
Signal,
```
plus its arm in `as_str()` (→ `"signal"`) and any `from_str`/match that must stay exhaustive. If a gated enum variant creates too much match-arm churn elsewhere, fall back to reusing `NodeKind::Event` (C#) and note the decision in the commit — do not invent a non-gated new variant that breaks other languages' exhaustiveness.

---

## Slices (each: build green + commit on `feat/gdscript-extractor`)

**Slice 0 — branch + vendor the grammar.**
- `git checkout -b feat/gdscript-extractor`.
- Vendor grammar: `git clone --depth 1 https://github.com/PrestonKnopp/tree-sitter-gdscript` then copy its `src/` (must include `parser.c`, `scanner.c`, and the `tree_sitter/` header dir) into `vendor/tree-sitter-gdscript/src/`. Do NOT keep the grammar's `.git`. Confirm `vendor/tree-sitter-gdscript/src/{parser.c,scanner.c,tree_sitter/parser.h}` exist.
- Gate: none yet; commit "vendor tree-sitter-gdscript grammar".

**Slice 1 — build wiring (compiles, extractor not yet registered).**
- `Cargo.toml`: add `lang-gdscript = []` in `[features]` and append `"lang-gdscript"` to the `full` list.
- `build.rs`: add a `CARGO_FEATURE_LANG_GDSCRIPT` block mirroring the **WGSL** block — compile `vendor/tree-sitter-gdscript/src/parser.c` **and** `scanner.c`, with `rerun-if-changed` for both.
- Gate: `cargo build --release --features lang-gdscript` succeeds (grammar object links even though nothing calls it yet — you may need a temporary `extern "C"` ref or just confirm the cc step runs; acceptable to fold this check into slice 2). Commit.

**Slice 2 — the extractor.**
- New `src/extraction/gdscript_extractor.rs`: `pub struct GdScriptExtractor;` implementing `LanguageExtractor` (`extensions()->["gd"]`, `language_name()->"GDScript"`, `extract(...)`). Mirror actionscript's internal `ExtractionState`/scope-stack/`extract_*` helpers. Load the grammar via the same `extern "C" { fn tree_sitter_gdscript() -> *const (); }` + `LanguageFn`/`set_language` pattern actionscript uses for `tree_sitter_actionscript` (see its lines ~230–245).
- Implement the mapping table above. Handle `has_error` trees gracefully (partial extraction + push to `errors`, never panic).
- Register in `mod.rs` (3 points, all `#[cfg(feature = "lang-gdscript")]`).
- Add `NodeKind::Signal` (gated) + `GDSCRIPT_COMPLEXITY`.
- Gate: `cargo build --release --features lang-gdscript` + `cargo clippy --features lang-gdscript` clean. Commit.

**Slice 3 — tests.**
- `tests/gdscript_extraction_test.rs`: small inline `.gd` fixtures asserting emitted nodes/edges — a script with `class_name`, `extends`, two `func`s (one `static`), a `signal`, a `const`, a class-level `var`, an inner `class`, an `enum`, and a call inside a func. Assert: 1 Class, correct Method/Function split, 1 Signal, 1 Const, ≥1 Field (and that a **local** var inside a func is NOT emitted as Field), Enum+variants, an Extends edge, ≥1 Calls edge.
- Gate: `cargo test --features lang-gdscript gdscript` green. Commit.

**Slice 4 — real-repo verification.**
- Build the default binary: `cargo build --release` (default features include `full` ⊇ `lang-gdscript`). First full build is slow (10–30 min); that's expected.
- Point it at the Godot project and index: use tokensave's own init/index CLI (check `--help`; likely `tokensave init` / re-sync) against `/home/kc/dev/godot-tactical-rpg`, in a scratch DB (do NOT clobber that repo's existing `.tokensave/tokensave.db` — use a temp dir or copy).
- Assert `tokensave status` (or the status JSON) now reports GDScript under `files_by_language` with a nonzero node count, and that node kinds include functions/classes/signals. Sanity vs spike oracle: expect thousands of functions, hundreds of extends/class_names, tens of enums/signals — same ballpark as the spike tallies (not exact; locals are excluded, methods vs functions split differs).
- Write results to `GDSCRIPT_EXTRACTOR_RESULTS.md` (status output + counts). Commit.

---

## Constraints & rules for the orchestrator
- **Commit on `feat/gdscript-extractor` only** (this is the fork; committing is authorized here — unlike the parent godot project). Readable messages, one per slice.
- **Touch only** tokensave files needed for the above + the new vendor/ grammar + the two new markdown files. No unrelated refactors, no dependency bumps beyond the vendored grammar.
- **Verify, don't assume:** paste actual `cargo build`/`test`/`clippy`/`status` output at each gate. A build "should pass" is not a pass.
- **Do not modify** the godot-tactical-rpg repo or its `.tokensave` DB, and do not touch its `.mcp.json` — wiring the new binary into that project is a SEPARATE step the human will approve after seeing Slice 4 results.
- If the gated `NodeKind::Signal` causes wide match-exhaustiveness breakage, take the `Event`-reuse fallback and note it — don't rabbit-hole.
- Report at the end: per-slice commit hashes, final `status` output proving GDScript is indexed, and any grammar gaps found (e.g. which real-world 4.x constructs, if any, still `has_error`).
```
