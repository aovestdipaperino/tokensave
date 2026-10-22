# Turso native FTS vs SQLite FTS5: methodology

Companion to [turso-fts-report.md](turso-fts-report.md). It covers what is
compared, how the workloads are kept equivalent, how results are judged, and
how to reproduce them. No production code or dependency is changed. The harness
lives in the standalone `benchmarks/db-compat` crate, which is not part of the
tokensave package or its CI.

## Questions

| | Question | Evidence |
| --- | --- | --- |
| A | Can turso run tokensave's existing SQL unchanged? | The SQL probes from #746 (`tokensave-db-compat`), unchanged. |
| B | Can turso's native full-text search implement tokensave's search requirements with adapted DDL and queries? | `fts_compare functional` and `fts_compare relevance`. |
| C | How does turso + Tantivy perform against libsql + FTS5 on equivalent workloads? | `fts_compare perf`. |
| D | Which blockers remain that have nothing to do with FTS? | A, plus the operational findings from B and C. |

A failure in A (`no such module: fts5`) says nothing about B. B is answered
only by a native turso implementation.

## What tokensave uses FTS5 for

Inventory taken from the base commit (`5357fe1`):

| Object | Definition | Kept in sync by | Read by |
| --- | --- | --- | --- |
| `nodes_fts` | `fts5(name, qualified_name, docstring, signature, search_terms, content='nodes', content_rowid='rowid', tokenize='porter unicode61')` (`migrations.rs` `create_schema`, v14 rebuild) | `nodes_fts_insert`/`_delete`/`_update` triggers; dropped by `begin_bulk_load` and repopulated with one `INSERT … SELECT` in `end_bulk_load`; `'rebuild'` on corruption (`rebuild_fts`) | `search_nodes_fts`, `search_nodes`, `search_nodes_phrase`, `search_nodes_bounded` (`db/queries/search.rs`) |
| `executable_body_fts` | `fts5(node_id UNINDEXED, file_path UNINDEXED, body, tokenize='unicode61')`, contentful | Application: delete by `file_path`, then prepared single-row inserts (`insert_executable_body_documents`) | `search_executable_bodies` |
| `memory_decisions_fts` | `fts5(text, reason, content='memory_decisions', content_rowid='id')` (default `unicode61`) | Three triggers | `session_recall` (`tokensave/memory.rs`) |

Query construction in production:

- `to_fts_match_query`: each whitespace word is stripped of `"` and sent as a
  quoted prefix term (`"word"*`), joined with `OR`.
- `to_fts_phrase_query`: multi-word input as one quoted phrase (the #362
  exact-phrase channel). `search_nodes` runs it first and merges the general
  results after it.
- Ranking: `ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0)` (column
  weights name, qualified_name, docstring, signature, search_terms). The score
  callers see is the negated rank, so higher is better.
- `search_nodes_bounded`: one quoted prefix term, excluding `file`/`doc` kinds,
  ranked, `LIMIT`.
- `search_executable_bodies`: `("abcde"* AND "fghij"*) OR …` over every pair
  of 4–5-character term prefixes, unranked, `LIMIT`.
- `session_recall`: `MATCH` with an optional `created_at >= ?`, ordered by
  `created_at DESC`, `LIMIT`.
- `search_nodes` falls back to `LIKE` when FTS returns nothing. The fallback is
  not part of either FTS arm and is never exercised by the harness.
- `search_terms` holds camelCase sub-words generated at insert time
  (`text::search_terms`), so camelCase splitting happens in the data, not in
  the tokenizer.

## Arms

| Label | Engine | Full-text implementation |
| --- | --- | --- |
| `libsql-fts5` (`LibsqlFts5Adapter`) | vendored libsql 0.9.30, bundled SQLite 3.45.1 | The three FTS5 objects and triggers above, verbatim, plus production's bulk-load steps and query shapes. |
| `turso-tantivy` (`TursoTantivyAdapter`) | turso 0.8.3 (`turso_core` 0.8.3, Tantivy 0.26.2), `experimental_index_method(true)` | `CREATE INDEX nodes_fts ON nodes USING fts (name, qualified_name, docstring, signature, search_terms) WITH (tokenizer='default', weights='name=10.0,qualified_name=5.0,docstring=1.0,signature=2.0,search_terms=3.0')`; `executable_body_fts` on a plain `executable_bodies(node_id, file_path, body)` table; `memory_decisions_fts` on `memory_decisions(text, reason)`. No triggers: the engine maintains the indexes on every INSERT/UPDATE/DELETE. |
| `turso-tantivy-simple` | same | Same DDL with `tokenizer='simple'` (keeps tokens over 40 bytes, does not lowercase). A configuration-sensitivity arm. |
| `turso-tantivy-ngram` | same | Same DDL with `tokenizer='ngram', min_gram=2, max_gram=8`. The only native stand-in for prefix queries, with substring semantics. An approximation arm. |

Turso's FTS is enabled with the builder flag the 0.8.x Rust API provides
(`experimental_index_method`); the crate's `fts` feature is on. FTS5
virtual-table syntax is never sent to turso. Turso's search never falls back to
`LIKE`, regular expressions, or filtering in the application.

Both arms share:

- the base tables (`nodes` with the FTS-relevant columns plus `kind` and
  `file_path`, an index on `file_path`, and `memory_decisions` as in
  production);
- the pragmas `page_size = 8192`, `journal_mode = WAL`, `synchronous = NORMAL`,
  `cache_size = -65536`, `temp_store = MEMORY`. `mmap_size` is left out for
  both, because turso accepts it without effect;
- statement shapes: nodes as 200-row multi-row `INSERT`s (production
  `insert_nodes`), bodies and decisions as single-row prepared inserts
  (production `insert_executable_body_documents`, `record_decision`);
- transaction boundaries (one per file edit, one per bulk load, autocommit for
  row updates);
- the process allocator. turso's default `mimalloc` feature installs a global
  allocator for the whole process. The crate disables it, so both arms use the
  system allocator, as production tokensave does. `--features turso-mimalloc`
  restores it for a sensitivity run.

### Query translation

Each query is written once as an engine-neutral intent (`src/bin/fts_compare/query.rs`)
and rendered per engine:

| Intent | FTS5 | Tantivy (turso) |
| --- | --- | --- |
| Term or phrase | `"text"` | `"text"` |
| Prefix, input with ≥ 2 tokens (`data-api`) | `"data-api"*` | `"data-api"*` (phrase prefix) |
| Prefix, single token (`pars`) | `"pars"*` | **unsupported**: `"pars"*` is a parse error (`PhrasePrefixRequiresAtLeastTwoTerms`) and `pars*` matches nothing without an error |
| AND / OR | `(a AND b)` / `(a OR b)` | same |
| NOT | `(a NOT b)` | `(+a -b)`. `a AND NOT b` returns no rows in Tantivy, because `NOT b` becomes a purely negative clause. |
| Column filter | `name : "x"` | `name:"x"` |

A single-token prefix is reported as `unsupported` unless a caller explicitly
asks for one of two labeled substitutions: `prefix-as-term` (exact term) in
the relevance runs, or the n-gram arm's substring match.

### Planner guard

turso decides from the statement's shape whether `fts_match`/`fts_score` run
through the index. A statement that matches none of its patterns (for example,
naming a subset of the indexed columns) runs them as scalar functions instead.
`fts_match` then means "shares any lowercase token" with no query syntax, and
`fts_score` returns 0.0. There is no error. The turso adapter runs `EXPLAIN` on
every distinct statement and refuses it unless the plan contains
`IndexMethodQuery`. Scenario 25 sends `fts_match(name, 'batching')` against
the five-column index; "batching" occurs only in a docstring. On 0.8.3 the
guard refuses the statement (hazard). On 0.9.0-pre.1 the index serves it and
honors the column list (pass, empty result).

### Recall query shape

On turso 0.8.3, `WHERE fts_match(…) ORDER BY created_at DESC LIMIT n` is
planned with the index method's LIMIT pattern. That pattern returns `n`
arbitrary matching rows and only then sorts them, so the result is wrong
(scenario 21a). The adapter's `recall` uses the equivalent subquery
`SELECT id FROM (SELECT id, created_at … WHERE fts_match(…)) ORDER BY
created_at DESC LIMIT n`. That keeps the index (checked by the guard) and
returns the correct rows (21b). Both shapes are reported.

## Datasets

`corpora.tsv` lists eight corpora with their pinned refs, commit SHAs and the
SHA-256 of each export:

| Corpus | Language | Size class | Nodes | Bodies |
| --- | --- | --- | --- | --- |
| requests v2.32.3 | Python | small | 1,582 | 667 |
| express 4.21.1 | JavaScript | small | 2,390 | 1,822 |
| fmt 11.0.2 | C++ | small | 7,106 | 3,090 |
| ripgrep 14.1.1 | Rust | small | 6,329 | 2,720 |
| vue core v3.5.12 | TypeScript | medium | 9,950 | 5,004 |
| tokensave `5357fe1` (no `vendor/`, `codegraph/`) | Rust | medium | 23,288 | 9,880 |
| redis 7.4.1 | C | medium | 26,628 | 12,807 |
| django 5.1.2 | Python | large | 68,688 | 29,211 |

Size classes: small < 8k nodes, medium 8k–30k, large > 30k.

Each corpus is indexed by `tokensave 7.15.0 init` and exported by
`fts_compare export`. The export holds the FTS-relevant node fields and the
executable bodies, ordered by id. Re-indexing a corpus from another directory
reproduced the export byte for byte (checked for requests). Every arm loads the
same JSONL rows in the same order. The corpora cover snake_case and camelCase
identifiers, qualified names (`::`, `.`, `/`), signatures, doc comments and
large files (vue core: 5.8 MB of bodies; django: 15 MB). Exports go to
`target/corpora/`, which is gitignored, and are about 100 MB in total. They are
not committed.

Synthetic parts, all derived with seed `0x7015E5A7EF75` (`corpus::SEED`, SplitMix64):

- **Memory decisions.** tokensave has no corpus of real ones. Each decision
  reuses a documented symbol's name, path and docstring excerpt. There are
  `nodes/20` decisions, clamped to 50–2000. `created_at` is shuffled against
  insertion order, so a query that orders by rowid instead of recency fails.
- **Incremental edit plan.** 50 files with at least 3 nodes each. 20% of their
  symbols are renamed and 20% get a new docstring, and one symbol is added per
  file. Each file is applied as production sync does: delete the file's rows,
  reinsert them, one transaction per file.
- **Functional fixture.** 18 hand-written nodes (snake_case, camelCase,
  acronyms, a 51-byte single-token identifier, German, Cyrillic, CJK, accented
  Latin, a `file` and a `doc` node), 3 bodies and 7 decisions
  (`functional.rs`).

## B. Functional correctness

Each scenario runs on a fresh copy of the fixture, built with the bulk path
(rows first, then the index), except scenario 23. Expectations are written
from the fixture text. **The libsql control must pass every scenario** (the
binary exits 1 otherwise), which validates the expectation. A turso deviation
is classified only when the observed result equals what a documented property
predicts:

| Class | Meaning |
| --- | --- |
| `pass` | Same result as the expectation. |
| `expected-semantic` | Follows from a property turso does not let you configure, such as no stemming or no diacritic folding. |
| `configurable` | Follows from the tokenizer chosen for that arm; another turso tokenizer changes it. |
| `approximation` | The n-gram arm returns the superset that substring matching predicts. |
| `unsupported` | turso cannot express the intent natively. |
| `hazard` | A silent wrong-semantics path that the harness detects. |
| `correctness-failure` | Anything else, including a predicted class whose predicted rows do not match. |

Several expectations and predictions were corrected while building the suite
(for example, porter stemming makes "Parsing" match `parse`). Each correction
was derived from the fixture text and the stated rule, not from the result the
turso arm returned.

## B. Relevance

Per corpus, 40 queries per intent are sampled with the fixed seed from the
corpus's own symbols. Kinds that are not symbols (`file`, `doc`, `use`,
`import`, `include`, `module`, `annotation_usage`, `decorator`,
`preprocessor_def`, `package`) are excluded.

| Intent | Query | Relevance rule (grade) |
| --- | --- | --- |
| `exact_name` | a symbol's name | 3: same name, case-insensitive; 1: name sub-words contain all of the query's |
| `partial_name` | one ≥ 4-letter sub-word of a multi-word name | 2: a sub-word of the name; 1: a sub-word of the qualified name or signature |
| `prefix` | the first 4 letters of a ≥ 6-letter name sub-word | 2: a name sub-word starts with it; 1: a sub-word of another field does |
| `qualified_name` | a qualified name with ≥ 3 tokens | 3: identical token sequence; 1: contains it contiguously |
| `signature` | 3 consecutive signature tokens | 2: the signature contains them contiguously |
| `doc_natural_language` | 3 content words of a docstring | 2: the docstring has all 3; 1: it has 2 (after stripping `-s/-es/-ed/-ing/-ly`) |
| `doc_natural_language_literal` | the same queries | the same rule without suffix stripping |
| `doc_phrase` | 2 adjacent content words of a docstring | 2: the docstring contains the bigram |
| `boolean_and` | first and last content word of a docstring, AND | 1: any indexed field contains both tokens |

Judgments are computed over every record of the corpus with plain
alphanumeric tokenization (plus camelCase splitting for identifiers), without
either engine. Two modes are run:

- `exact`: the intent as quoted terms, phrases and booleans. Both engines
  support this, so differences come from tokenization and ranking.
- `production`: the input passed through production's `search_nodes`
  construction (phrase channel plus an OR of prefixes). Unsupported queries
  count as empty results. The arm `turso-tantivy/prefix-as-term` shows the
  degraded rewrite.

Metrics at k = 10 over queries with at least one relevant record:
Recall@5/10 (relevant retrieved / all relevant), Precision@5, MRR@10, and
NDCG@10 with gain 2^g − 1. Agreement with libsql: Jaccard overlap of the top 10
and rank-biased overlap (p = 0.9). These are not relevance measures: libsql is
not ground truth. Stability: whether each exact-mode query returns an
identical list on a second run and after closing and reopening the database.

## C. Performance

`fts_compare perf` runs each phase in a fresh child process
(`perf-child <phase> <adapter> <corpus> <dir>`). Each child prints its timings
and `VmRSS`/`VmHWM` from `/proc/self/status`, so each peak belongs to one
engine and one phase. The order of the two engines is swapped every
repetition. Query, startup, update and concurrency phases start from a byte
copy of one database built by the bulk phase.

| Phase | What is timed |
| --- | --- |
| `build-bulk` | Production bulk path: load rows (per table), then build FTS (libsql: triggers plus `INSERT … SELECT`; turso: `CREATE INDEX … USING fts` over existing rows). libsql's `executable_body_fts` is contentful, so its index is built during the row load. Compare `total_ms`. |
| `build-incremental` | FTS live from the start, all rows in one transaction, split per table. |
| `startup-cold-cache` / `-warm-cache` | A fresh process opens the database (pragmas included) and runs the first and second query. Before the cold run the parent `fsync`s and `posix_fadvise(DONTNEED)`s the files to drop them from the OS page cache. The warm run follows on the same files. |
| `query` | One unmeasured warm-up pass over all queries, then 3 measured passes. Categories: exact name, prefix, phrase (qualified/signature/doc phrase), complex (boolean AND, 3-word OR), bounded (kind filter), production `search_nodes`, memory recall, executable-body pairs (exact terms), and one statement repeated 500×. Steady-state RSS is read after the passes. |
| `update` | 50 file edits (one transaction each), up to 100 autocommit row updates, up to 20 file deletes. Each is followed by correctness checks (added symbols found, renamed found, new docstring text found, deleted not found). Then exact-name latency on the edited index, `optimize` (FTS5 `'optimize'` / `OPTIMIZE INDEX`), and latency again. |
| `concurrency` | The exact-name workload on 1 connection, then on 4 connections (one tokio task each, same process; libsql opens a second local `Database`, turso calls `Database::connect`). Both engines support this model. |

Unsupported queries are counted and not timed. A category on one engine is
compared only with the queries that engine could run. Results are summarized as
the median across repetitions with the [min–max] range. A turso/libsql ratio is
prefixed with `~` when the two ranges overlap, and such a difference is not
claimed.

## Biases and limitations

- **The judgments are rule-based** and favor lexical identifier matching,
  which is what tokensave's symbol search is for. The natural-language rule
  strips suffixes, which favors a stemming engine; the `_literal` variant
  shows by how much.
- **One machine**: Linux on WSL2, 8 logical CPUs, 7 GiB RAM. The OS page cache
  of the WSL2 VM is controlled; the Windows host's cache is not.
- **Synthetic workload slices**: memory decisions and edits are synthetic, and
  the query mix is not taken from real usage logs.
- **Ranking is compared per statement, not per tokensave feature.** Context
  building re-ranks FTS candidates with other signals, and that pipeline is not
  replayed here.
- **Tokenizer mismatch can't be removed.** FTS5 `porter unicode61` stems and
  folds diacritics. The turso arms do neither. Tantivy's `en_stem` exists, but
  turso 0.8.3 rejects it (`unsupported FTS tokenizer`). BM25 implementations
  differ (per-field statistics in Tantivy, per-row in FTS5), so scores are
  never compared, only rankings.
- **No multi-process test.** Concurrency is in-process only.

## Reproduce

```sh
cd benchmarks/db-compat

# 0. Original SQL probes (A), unchanged
cargo build --release
./target/release/tokensave-db-compat --existing-db target/real-v18.db --json /tmp/probes.json

# 1. Corpora (needs network and tokensave 7.15.0 on PATH); prints hash checks
scripts/prepare-fts-corpora.sh ~/tokensave-fts-bench-cache

# 2. Functional suite (B), seconds
./target/release/fts_compare functional --json results/fts/functional-turso-0.8.3.json

# 3. Relevance (B), a few minutes
./target/release/fts_compare relevance target/corpora/*.jsonl --json results/fts/relevance-turso-0.8.3.json

# 4. Performance (C): one result file per corpus, 3 repetitions; about
#    25 minutes in total on the reference machine (django alone ~13 min).
#    Keep the machine idle.
for c in requests express fmt ripgrep vue-core tokensave redis django; do
  ./target/release/fts_compare perf target/corpora/$c.jsonl --reps 3 \
      --json results/fts/perf-turso-0.8.3-$c.json
done

# 5. Allocator sensitivity (optional)
CARGO_TARGET_DIR=target-mimalloc cargo build --release --bin fts_compare --features turso-mimalloc
for c in fmt redis; do
  target-mimalloc/release/fts_compare perf target/corpora/$c.jsonl --reps 3 \
      --json results/fts/perf-mimalloc-turso-0.8.3-$c.json
done

# 6. Summaries (CSV + Markdown)
python3 -I scripts/summarize_fts.py results/fts

# Other turso releases: change the `turso` pin in Cargo.toml (labels and the
# environment block take the version from Cargo.lock), then rerun step 2.
```

`fts_compare env` prints the recorded environment block. `cargo test --release
--bin fts_compare` runs the metric unit tests.
