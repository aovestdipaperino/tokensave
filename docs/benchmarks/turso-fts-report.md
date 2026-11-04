# Turso native FTS (Tantivy) vs SQLite FTS5 for tokensave

Status: evaluation for #739. Nothing is migrated. The harness is
`benchmarks/db-compat` (`fts_compare`), the method is in
[turso-fts-methodology.md](turso-fts-methodology.md), and the raw results are in
[`benchmarks/db-compat/results/fts/`](../../benchmarks/db-compat/results/fts/),
including the generated [`summary.md`](../../benchmarks/db-compat/results/fts/summary.md)
and CSV files.

## 1. Executive summary

The SQL probes (#746) showed that turso can't run tokensave's FTS5 SQL. This
report answers a separate question: can turso's **native** full-text search
(`CREATE INDEX … USING fts`, Tantivy 0.26.2) be adapted to do what tokensave's
FTS5 search does, and how does it perform?

**Functionally: partly.** With adapted DDL and queries, turso passes 31 of 41
scenarios on 0.8.3. Exact terms, phrases, booleans, multi-column
weighted BM25, Unicode identifiers, updates, deletes, transaction visibility
and reopen all work, with the engine keeping the index in sync and no
triggers. What it can't do:

1. **Single-token prefix queries** (`"pars"*`). All four production call sites
   send them: `search_nodes`, `search_nodes_bounded`,
   `search_executable_bodies` and `session_recall`. Tantivy's parser rejects
   `"pars"*` and returns nothing for `pars*` without an error. The only native
   alternative, an n-gram index, matches substrings and breaks phrase queries.
2. **Stemming and diacritic folding.** turso 0.8.3 and 0.9.0-pre.1 reject
   `en_stem`, and no tokenizer folds accents.
3. **Tokenizer trade-off.** The `default` tokenizer drops tokens over 40 bytes
   (117 django nodes, 18 redis names). `simple` keeps them but makes search
   case-sensitive. No tokenizer gives both.

There are also two correctness hazards on 0.8.3:

- The production recall shape (`ORDER BY created_at DESC LIMIT n`) returns
  wrong rows: the limit is applied before the sort. A subquery form fixes it.
- A statement the planner does not route to the index runs `fts_match` as a
  naive scalar with no error. The harness detects this with `EXPLAIN`.
  0.9.0-pre.1 serves that shape through the index correctly.

**Relevance: mixed, and the difference is not stemming.** On identifier
intents with exact terms, turso's ranking is as good as or better than FTS5
against independent judgments (for example, exact-name NDCG@10 0.967 vs 0.894).
On natural-language docstring search it is much worse (NDCG@10 0.124 vs 0.500,
and 0.125 vs 0.511 with unstemmed judgments). Tantivy's per-field BM25 ranks a
short name that contains one query word above a docstring that contains all of
them, and uniform weights do not change that. In production mode (prefix
queries) turso can answer only queries whose words contain punctuation.

**Performance (3 repetitions, 8 corpora):** turso is not uniformly
slower, but its costs fall where tokensave is most sensitive:

- **Single-row writes into FTS-indexed tables are much slower.** turso
  builds a segment per statement. Row updates are 94–156× slower (4–9 ms
  against about 0.05 ms), and single-row body inserts 310–640× slower. A first
  index with the FTS index live takes 41–90× longer: 203 s against 4.4 s on
  django.
- **Median (p50) exact-name query latency is 1.9–6.2× higher.** The first
  query after open takes 1.7–39 ms against 0.2–2 ms (fmt: 5.5–6.9 ms). Tail
  latency (p99) is often lower than FTS5's on broad queries.
- **The file-edit sync path is 2.6–55× slower at p50**, and search latency
  after edits rises (1.1–7.9×) until `OPTIMIZE INDEX`.
- **What is better on turso:** FTS structures take 31–64% of FTS5's space,
  and reads scale 2.2–3.9× across 4 connections (FTS5 does not, in this
  in-process model).
- **Bulk index builds range from 13% faster to 59% slower,** and RSS is
  11–41 MiB higher.

**Recommendation:** do not migrate. Single-token prefix search, which every
production FTS call depends on, has no native equivalent, and single-row
writes into an FTS-indexed table are orders of magnitude slower. Revisit when
turso exposes prefix (or edge-n-gram) queries and a stemming tokenizer, and
amortizes per-statement segment builds. The harness is ready to rerun (§10).

## 2. Versions

| Component | Version |
| --- | --- |
| libsql (control) | 0.9.30, vendored with the #367 patch; bundled SQLite 3.45.1, FTS5 |
| turso | 0.8.3, the latest stable release (crates.io, 2026-10-12); `turso_core` 0.8.3 |
| Tantivy (inside turso) | 0.26.2 |
| turso prerelease | 0.9.0-pre.1 (2026-10-15): functional suite only |
| tokensave used to index corpora | 7.15.0 (schema v18) |
| rustc | 1.98.1, release profile |
| Allocator | system for both arms; turso's `mimalloc` default feature as a separate sensitivity run |
| Machine | 11th Gen Intel Core i7-1165G7, 8 logical CPUs, 7.6 GiB RAM, Ubuntu 26.04.1 LTS on WSL2 (kernel 6.18.33.2) |

Should newer versions be tested? 0.9.0-pre.1 changes one result: scenario 25
passes. Its FTS surface is otherwise the same: same tokenizer allow-list, same
phrase-prefix rule, same recall-ordering bug. It still cannot open existing
tokensave databases (#746), so it was not benchmarked. Rerun the functional
suite and then perf on the next stable release (§10).

## 3. Implementations compared

**libsql + FTS5** (`LibsqlFts5Adapter`) uses production's DDL verbatim:

- external-content `nodes_fts` (`porter unicode61`, 5 columns), kept in sync
  by triggers;
- contentful `executable_body_fts` (`unicode61`);
- `memory_decisions_fts` with triggers;
- production's bulk path (drop triggers, load, `INSERT … SELECT`), and
  production's query and ranking SQL (`bm25(nodes_fts, 10, 5, 1, 2, 3)`).

**turso + Tantivy** (`TursoTantivyAdapter`) uses
`CREATE INDEX nodes_fts ON nodes USING fts (name, qualified_name, docstring,
signature, search_terms) WITH (tokenizer='default', weights='name=10.0,…')` and
the matching indexes on `executable_bodies(body)` and
`memory_decisions(text, reason)`. It is enabled by
`Builder::experimental_index_method(true)` with the crate's `fts` feature.

- The engine maintains the indexes on every DML statement; there are no
  triggers. Writes flush one immutable segment per statement, and segments are
  merged above 32 (`PRAGMA fts_merge_threshold`).
- Queries use `fts_match(cols…, q)` and `fts_score(cols…, q)` with
  `ORDER BY score DESC LIMIT n`.
- FTS5 syntax is never sent to turso, and there is no `LIKE` or application
  filtering.

Two more turso arms test configuration: `turso-tantivy-simple` (keeps long
tokens, case-sensitive) and `turso-tantivy-ngram` (2–8-grams, substring
matching).

Differences that can't be made equivalent:

- Tokenization: no stemming, no diacritic folding, and the 40-byte limit
  (above).
- `NOT`: FTS5 `a NOT b` corresponds to Tantivy `+a -b`, because Tantivy's
  `a AND NOT b` returns no rows.
- BM25: Tantivy keeps statistics per field, FTS5 per row. Scores are never
  compared, only rankings.
- camelCase splitting comes from production's `search_terms` column, so both
  engines get it.

## 4. Functional compatibility

41 scenarios on a hand-built fixture. Each expectation was derived from the
fixture text, and the libsql control passes all 39 that apply to it.
Full table:
[`functional-turso-0.8.3.md`](../../benchmarks/db-compat/results/fts/functional-turso-0.8.3.md).

| Arm (turso 0.8.3) | pass | expected-semantic | configurable | approximation | unsupported | hazard | correctness-failure |
| --- | --- | --- | --- | --- | --- | --- | --- |
| turso-tantivy | 31 | 3 | 1 | – | 4 | 1 | 1 |
| turso-tantivy-simple | 27 | 3 | 5 | – | 4 | 1 | 1 |
| turso-tantivy-ngram | 32 | 2 | – | 3 | 1 | 1 | 2 |

The 41 rows include the two turso-only checks (25 and 26).

**Passes on turso-tantivy:**

- exact (1), camelCase via `search_terms` (2), multi-token phrase prefix (3b),
  qualified name (4), signature (5), OR-of-words documentation search (6),
  phrase (7);
- AND, OR and NOT, including NOT over phrase prefixes (8a–d);
- snake_case, camelCase, digits, case and Cyrillic case folding, Latin and CJK
  identifiers (9a–c, 10, 11a/c/d);
- update and delete (12, 13), visible after commit from another connection
  (14), visible inside the writer's open transaction and gone after rollback
  (15);
- name weight outranks docstring (16), ranked-result contract (17), bounded
  search with the kind filter (18), the incremental index path (23), reopen
  (24).

**Not passing:**

| # | Scenario | turso-tantivy | Why |
| --- | --- | --- | --- |
| 3 | `"pars"*` | unsupported | Tantivy needs ≥ 2 tokens for a phrase prefix. `pars*` returns no rows and no error. |
| 19 | production `search_nodes("parse config")` | unsupported | Its OR channel is single-token prefixes. |
| 22a | production body search (`"preco"* AND "matri"*`) | unsupported | Same. |
| 26 | `tokenizer='en_stem'` | unsupported | `unsupported FTS tokenizer 'en_stem'. Supported tokenizers: default, raw, simple, whitespace, ngram` |
| 6b, 20 | "running" / `name : parse` | expected-semantic | No stemming. |
| 11b | "cafe" vs "café" | expected-semantic | No diacritic folding. |
| 9d | a 51-byte identifier | configurable | `default` drops it; `simple` finds it but is case-sensitive (2, 8b, 10a/b, 11d fail on that arm). |
| 21a | recall, production shape | **correctness-failure** | Returned `[2,1]` instead of `[2,7]`: the LIMIT pattern takes arbitrary matches before sorting. 21b, the subquery form, passes and is what the adapter uses. Same on 0.9.0-pre.1. |
| 25 | `fts_match(name, …)` on a 5-column index | hazard (0.8.3) / pass (0.9.0-pre.1) | 0.8.3 plans it without the index: scalar any-token match, score 0.0, no error. The guard refuses it. 0.9.0-pre.1 serves it from the index and honors the column. |

The n-gram arm turns the prefix scenarios into substring supersets
(approximation) and runs production `search_nodes` and body pairs. It fails 8d
because n-grams keep separators: `"run loo"*` cannot match `run_loop`.

## 5. Search relevance

40 queries per intent per corpus, about 2,500 per arm in exact mode over 8
corpora. Judgments are rule-based and independent of both engines; libsql's
ranking is not ground truth. Macro-averages over corpora, from
[`summary.md`](../../benchmarks/db-compat/results/fts/summary.md):

**Exact mode** (quoted terms, phrases and booleans that both engines support):

| Intent | NDCG@10 libsql / turso | MRR@10 | R@10 | P@5 | overlap@10 | RBO |
| --- | --- | --- | --- | --- | --- | --- |
| exact_name | 0.894 / 0.967 | 0.972 / 1.000 | 0.762 / 0.774 | 0.472 / 0.498 | 0.84 | 0.79 |
| partial_name | 0.809 / 0.990 | 0.851 / 1.000 | 0.263 / 0.316 | 0.773 / 0.941 | 0.46 | 0.48 |
| qualified_name | 0.979 / 0.997 | 1.000 / 0.997 | 0.986 / 0.983 | 0.280 / 0.279 | 0.98 | 0.93 |
| signature | 0.988 / 0.998 | 0.987 / 0.998 | 0.884 / 0.887 | 0.506 / 0.514 | 0.90 | 0.84 |
| doc_phrase | 0.833 / 0.891 | 0.803 / 0.885 | 0.875 / 0.901 | 0.346 / 0.382 | 0.87 | 0.83 |
| boolean_and | 0.830 / 1.000 | 0.822 / 1.000 | 0.806 / 0.884 | 0.457 / 0.575 | 0.68 | 0.66 |
| doc_natural_language | **0.500 / 0.124** | 0.637 / 0.159 | 0.397 / 0.139 | 0.295 / 0.078 | 0.25 | 0.28 |
| doc_natural_language_literal | **0.511 / 0.125** | 0.611 / 0.146 | 0.445 / 0.152 | 0.269 / 0.069 | 0.25 | 0.28 |

- On identifier intents turso ranks at least as well. FTS5's porter stemming
  also pulls in morphological variants (for example `parse` → `parsing`) that
  these lexical rules don't count as relevant.
- On natural-language docstring queries (3 words, OR) turso is far behind, and
  removing stemming from the judgments changes almost nothing.
- Diagnostic (requests, "authentication header credentials"): turso's top 6
  are all `HEADER_VALIDATORS…` names. FTS5's top 6 are auth helpers whose
  docstrings match. With uniform weights turso still puts five
  `HEADER_VALIDATORS…` names in its top 6, so the cause is per-field BM25
  normalization, not the weight values.
- Overlap with libsql's top 10 ranges from 0.25 to 0.98, so a port changes
  what users see even where quality holds.

**Production mode** (production's prefix query construction):

| Intent | libsql | turso strict (unsupported) | turso prefix-as-term | turso ngram |
| --- | --- | --- | --- | --- |
| exact_name | 0.881 | 0.394 (187/320) | 0.952 | 0.949 |
| partial_name | 0.764 | 0 (320/320) | 0.990 | 0.908 |
| prefix | 0.960 | 0 (320/320) | **0.163** | 0.937 |
| qualified_name | 0.976 | 0.990 (0/320) | 0.990 | 0.994 |
| signature | 0.988 | 0 (320/320) | 0.998 | 0.618 |
| doc_phrase | 0.833 | 0 (320/320) | 0.891 | 0.668 |
| doc_natural_language | 0.483 | 0 (320/320) | 0.127 | 0.155 |
| boolean_and (as OR input) | 0.530 | 0 (293/293) | 0.272 | 0.264 |

NDCG@10. Unsupported queries count as empty results.

- Strict turso answers only inputs whose words contain punctuation (multi-token
  phrase prefixes such as qualified names).
- Dropping the prefix operator keeps exact intents intact but destroys prefix
  search.
- The n-gram arm restores prefix search but loses phrase and signature
  precision.
- No turso configuration matches FTS5 on all intents.

**Stability:** every arm returned identical top-10 lists on repeat and after
reopening, for all 2,533 exact-mode queries.

**Long tokens** (dropped by turso `default`), as nodes with an indexed
alphanumeric run over 40 bytes / with one in the name:

- django 117 / 33, redis 20 / 18, fmt 8 / 7, vue core 2 / 1, requests 1 / 0;
- 0 in express, ripgrep and tokensave.

## 6. Performance

All numbers are medians of 3 repetitions in fresh processes, shown as
libsql / turso. Per-repetition ranges are in
[`summary.md`](../../benchmarks/db-compat/results/fts/summary.md) and the
CSV files. Every update, rename, docstring-update and delete check passed on
both engines for every corpus. Ratios marked `~` in `summary.md` have
overlapping ranges and are not claimed.

**Indexing**

| Corpus (nodes) | Bulk build, ms | Incremental first index, ms | …of which single-row body inserts, ms |
| --- | --- | --- | --- |
| requests (1.6k) | 54 / 86 | 68 / 4,903 | 9 / 4,327 |
| express (2.4k) | 129 / 168 | 160 / 11,250 | 33 / 10,360 |
| fmt (7.1k) | 313 / 272 | 253 / 12,090 | 16 / 10,020 |
| ripgrep (6.3k) | 164 / 231 | 199 / 8,077 | 22 / 6,779 |
| vue core (10k) | 290 / 368 | 312 / 20,810 | 56 / 18,120 |
| tokensave (23k) | 584 / 805 | 677 / 48,670 | 99 / 43,130 |
| redis (27k) | 749 / 1,088 | 886 / 79,400 | 132 / 69,560 |
| django (69k) | 3,565 / 3,363 | 4,423 / 203,200 | 469 / 187,600 |

- The bulk path (rows first, then `CREATE INDEX … USING fts` or FTS5
  `INSERT … SELECT`) is comparable.
- With the index live, each single-row INSERT builds a Tantivy segment. The
  inserts use production's statement shapes: nodes in 200-row INSERTs, bodies
  and decisions one row per statement.

**Queries** (µs, warm, after a warm-up pass)

| Corpus | exact name p50 | exact name p99 | phrase p50 | boolean/OR p50 | boolean/OR p99 | kind-filtered p50 | memory recall p50 | body pairs p50 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| requests | 77 / 147 | 282 / 546 | 77 / 147 | 93 / 191 | 1,309 / 339 | 78 / 188 | 22 / 92 | 50 / 113 |
| express | 33 / 204 | 1,232 / 381 | 63 / 234 | 127 / 269 | 526 / 865 | 101 / 353 | 32 / 200 | 168 / 302 |
| fmt | 56 / 179 | 3,643 / 413 | 104 / 242 | 159 / 240 | 4,403 / 875 | 119 / 282 | 24 / 85 | 78 / 140 |
| ripgrep | 76 / 163 | 530 / 270 | 126 / 170 | 118 / 205 | 1,966 / 406 | 135 / 224 | 28 / 82 | 92 / 144 |
| vue core | 36 / 135 | 850 / 272 | 93 / 239 | 111 / 252 | 5,504 / 480 | 197 / 326 | 29 / 84 | 222 / 235 |
| tokensave | 161 / 403 | 1,003 / 936 | 189 / 458 | 370 / 467 | 4,716 / 938 | 253 / 475 | 26 / 86 | 329 / 337 |
| redis | 80 / 258 | 4,149 / 696 | 363 / 528 | 412 / 655 | 4,567 / 1,172 | 475 / 719 | 30 / 143 | 335 / 418 |
| django | 296 / 1,252 | 2,987 / 3,210 | 410 / 1,793 | 628 / 1,412 | 42,260 / 2,748 | 1,353 / 1,552 | 34 / 113 | 362 / 532 |

- turso's medians are higher everywhere.
- Its p99 is lower than FTS5's on broad OR/boolean queries and many name
  queries. FTS5 must score every match before `ORDER BY bm25 LIMIT`, while
  Tantivy collects the top k. The worst FTS5 case is 42 ms on django.
- Prefix queries (the production default) can't be timed on turso
  (40/40 unsupported per corpus).
- Production `search_nodes` was timed only on the subset turso can express.

**Startup**, fresh process, open plus first query, ms:

| | requests | vue core | tokensave | redis | django |
| --- | --- | --- | --- | --- | --- |
| open, cold page cache | 0.6 / 2.6 | 0.5 / 1.4 | 0.5 / 1.2 | 0.6 / 1.6 | 1.1 / 3.9 |
| first query, cold | 0.5 / 2.1 | 0.3 / 4.2 | 0.3 / 10.3 | 0.4 / 13.2 | 2.0 / 38.8 |
| first query, warm | 0.5 / 1.7 | 0.2 / 4.4 | 0.3 / 7.5 | 0.2 / 11.0 | 1.6 / 33.8 |

turso loads every visible segment into memory on the first FTS query, so the
cost grows with index size. This matters for short-lived CLI and git-hook
processes.

**Updates and sync** (ms unless noted)

| Corpus | file edit p50 | file edit p95 | row update p50 | file delete p50 | exact name p50 after edits (µs) | optimize | after optimize (µs) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| requests | 1.6 / 27.6 | 17.9 / 318 | 0.05 / 7.0 | 0.2 / 0.9 | 138 / 452 | 2.5 / 25.5 | 61 / 156 |
| fmt | 2.0 / 21.1 | 31.3 / 1,101 | 0.04 / 5.3 | 0.7 / 1.9 | 98 / 986 | 8.6 / 48.8 | 53 / 200 |
| vue core | 2.0 / 11.4 | 6.7 / 75.1 | 0.04 / 4.1 | 1.8 / 2.0 | 43 / 479 | 9.4 / 74.6 | 35 / 231 |
| tokensave | 4.3 / 44.5 | 22.3 / 242 | 0.04 / 5.8 | 3.6 / 3.9 | 351 / 1,406 | 21.1 / 193 | 125 / 476 |
| redis | 8.6 / 44.6 | 24.3 / 218 | 0.10 / 9.0 | 7.2 / 7.1 | 182 / 2,041 | 54.8 / 682 | 75 / 836 |
| django | 10.8 / 28.5 | 22.2 / 609 | 0.05 / 7.1 | 8.7 / 2.2 | 535 / 1,381 | 59.3 / 309 | 210 / 585 |

- Deletes write tombstones and are as fast as, or faster than, FTS5.
- After 50 file edits, turso's query latency rises 1.1–7.9× until an
  explicit `OPTIMIZE INDEX`. Write-path merges (threshold 32 segments) don't fully
  recover it.

**Repeated query and concurrency** (q/s)

| Corpus | repeated single query | 1 connection | 4 connections |
| --- | --- | --- | --- |
| requests | 16,330 / 5,605 | 9,423 / 3,579 | 13,720 / 12,570 |
| fmt | 271 / 3,161 | 2,948 / 5,761 | 1,703 / 15,300 |
| vue core | 30,480 / 7,651 | 9,591 / 4,714 | 7,802 / 13,970 |
| redis | 26,720 / 3,576 | 4,782 / 2,181 | 3,247 / 8,502 |
| django | 1,868 / 582 | 1,810 / 700 | 1,535 / 1,511 |

- The repeated query is the first sampled name per corpus; on fmt it is
  slow for FTS5.
- turso's reads scale 2.2–3.9× from 1 to 4 connections (django: 2.2×).
  libsql's in-process local connections do not scale here, and often drop.
- tokensave's MCP server uses one connection, so the 1-connection column is
  the relevant one today.

**Allocator sensitivity** (`--features turso-mimalloc`, fmt and redis, 3
repetitions, both arms under mimalloc):

- Timings move by up to about 2× in either direction for both engines. For
  example, turso's redis incremental first index went from 79 s to 52 s, and
  its row update from 9.0 to 4.6 ms. libsql's fmt bulk build went from 313 to
  166 ms.
- Steady-state RSS rises by 10–26 MiB for both engines.
- No conclusion above changes. Turso's row updates stay about 85–110× slower,
  and its exact-name p50 stays 1.9–5.6× higher.

## 7. Memory and storage

| Corpus | Disk total, MiB | FTS structures, MiB | Peak RSS bulk build, MiB | Steady-state RSS after queries, MiB |
| --- | --- | --- | --- | --- |
| requests | 1.5 / 1.5 | 0.8 / 0.4 | 14 / 35 | 14 / 25 |
| express | 3.2 / 3.2 | 2.3 / 0.7 | 18 / 47 | 17 / 30 |
| fmt | 5.1 / 5.4 | 2.4 / 1.5 | 23 / 51 | 21 / 34 |
| ripgrep | 4.6 / 4.8 | 2.5 / 1.3 | 22 / 49 | 20 / 33 |
| vue core | 14.2 / 14.6 | 9.5 / 3.0 | 41 / 67 | 36 / 52 |
| tokensave | 23.3 / 24.1 | 15.3 / 6.0 | 59 / 87 | 53 / 71 |
| redis | 25.8 / 27.2 | 15.3 / 8.1 | 65 / 89 | 56 / 79 |
| django | 60.5 / 63.2 | 31.5 / 16.4 | 139 / 180 | 115 / 151 |

- FTS structures: libsql's figure is the `dbstat` size of the three FTS5
  objects. It includes `executable_body_fts`'s stored body text, which turso
  keeps in a plain table instead. turso's figure is the file-size delta of
  the bulk index build.
- Total file sizes, which hold the same logical content, are within 5%.
- RSS: turso uses 11–41 MiB more in every phase. Its FTS reader keeps the
  visible segments resident (192 MiB retained-cache budget in
  `turso_core`), so the gap grows with index size.
- After the update workload, file sizes stay within 5% of each other.
- Memory stabilization was not tested beyond one query phase per process.
  Each phase is one process, so a leak would show only within a phase.

## 8. Remaining migration blockers

### A. Direct SQL compatibility (#746, unchanged)

Turso 0.8.3 runs 39 of 55 SQL probes with SQLite's results. The full table is
in [turso-compatibility.md](turso-compatibility.md).

### B. Search functionality (this report)

| Blocker | Severity | Evidence |
| --- | --- | --- |
| No single-token prefix query | **Blocker.** Every production FTS call uses it. | Scenarios 3, 19, 22a; production-mode relevance |
| No stemming or diacritic folding | High for natural-language search | 6b, 11b, 20, 26 |
| Natural-language ranking far below FTS5 | High for `tokensave_context` documentation queries | §5 |
| `default` drops tokens over 40 bytes; `simple` is case-sensitive | Medium (rare in these corpora) | 9d, 10, 11d; long-token counts |
| Recall shape returns wrong rows | Medium. A subquery workaround exists. | 21a/21b |
| Silent scalar fallback when the planner misses the index | Medium on 0.8.3 (needs an `EXPLAIN` guard). Fixed in 0.9.0-pre.1. | 25 |
| FTS behind `experimental_index_method` | Operational risk | builder flag |

### C. Performance

| Blocker | Evidence |
| --- | --- |
| Per-statement segment builds make single-row writes into FTS-indexed tables much slower: updates 94–156×, body inserts 310–640×. Production body inserts and memory writes are single-row; batching them would be a required redesign. | Incremental build; row updates |
| Sync-path file edits are 2.6–55× slower at p50; query latency degrades after edits until `OPTIMIZE INDEX` | Updates table |
| First FTS query per process loads all segments (11–39 ms on medium/large corpora) | Startup table |
| Median query latency 1.1–6.2× higher (exact name: 1.9–6.2×); RSS 11–41 MiB higher | §6, §7 |

### D. Blockers unrelated to FTS (from #746)

- `PRAGMA integrity_check` reports `Page N: never used` on a valid tokensave
  database (0.8.3), which would break `verify.rs`.
- 0.9.0-pre.1 refuses to open existing tokensave databases (`Virtual table
  module not found: fts5`).
- There is no in-place migration path for existing databases: FTS5 virtual
  tables and shadow tables would need a rebuild.
- `PRAGMA mmap_size` is accepted without effect.
- `execute_batch` rejects row-returning pragmas, so `apply_pragmas` and
  `begin_bulk_load` would need restructuring.
- `VACUUM` needs `experimental_vacuum` and conflicts with multi-process WAL.
- Not tested: multi-process access (CLI, git hooks and the MCP server on one
  file), busy timeouts under contention, crash recovery.
- New from this work: turso's default `mimalloc` feature replaces the global
  allocator of the whole tokensave process.

## 9. Limitations and potential biases

- **Relevance judgments are rule-based and lexical.** They are independent of
  both engines but written by the harness author. The natural-language rule
  strips suffixes, which favors stemming; the `_literal` variant controls for
  it.
- **FTS5's query shapes and BM25 weights were tuned for FTS5** (#264, #313,
  #362). A turso port would retune them, and the weights tested do not
  explain the natural-language gap.
- **Part of the workload is synthetic:** memory decisions, the edit plan and
  the query mix. There are no usage logs.
- **One machine** (WSL2, 8 threads, 7 GiB): the Windows host page cache is
  not controlled. Multi-process access is not tested.
- **turso is experimental and changing quickly** (nine published versions,
  prereleases included, between 2026-09-29 and 2026-10-15). These results describe 0.8.3, plus the 0.9.0-pre.1
  functional run.

## 10. Recommendation

**Do not migrate tokensave's search to turso + Tantivy now.** The evidence:

1. **Functional gaps that affect every production FTS call.** There is no
   single-token prefix query. Stemming and diacritic folding are unavailable,
   and the long-token versus case-sensitivity trade-off has no tokenizer that
   avoids both. Turso is *functionally capable* of exact-term, phrase,
   boolean, weighted, transactional search. It is *not* a semantic
   replacement for tokensave's current search contract.
2. **Ranking:** competitive or better on identifier lookups. Far worse on
   natural-language docstring queries, which `tokensave_context` relies on.
3. **Write path:** per-statement segment builds make the single-row writes
   tokensave performs (bodies, memory decisions, row updates) two to three
   orders of magnitude slower. Sync edits are 2.6–55× slower at p50.
4. **Read path:** p50 1.9–6.2× slower on exact names, first query per process
   up to 39 ms. turso wins on tail latency for broad queries, on FTS size and
   on multi-connection read scaling.
5. **Non-FTS blockers** from #746 remain: `integrity_check`, migration of
   existing databases, and the prerelease being unable to open them.

What would justify another look:

- turso exposes prefix or edge-n-gram queries and a stemming tokenizer;
- it amortizes single-row FTS writes, or tokensave batches them;
- 0.9.x stabilizes.

Rerunning `fts_compare functional` against a new release takes seconds and
shows the first two directly. The perf and relevance suites then quantify the
rest without code changes. More investigation would also be needed on
multi-process access and on re-tuning ranking for Tantivy's per-field BM25
before any port.

Reproduce: [turso-fts-methodology.md § Reproduce](turso-fts-methodology.md#reproduce).
