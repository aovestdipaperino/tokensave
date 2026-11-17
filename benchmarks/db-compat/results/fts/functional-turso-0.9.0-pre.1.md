| # | Scenario | libsql-fts5 | turso-tantivy | turso-tantivy-simple | turso-tantivy-ngram |
|---|---|---|---|---|---|
| 1 | Exact symbol name lookup | pass | pass | pass | pass |
| 2 | Partial symbol lookup (camelCase inner word via search_terms) | pass | pass | configurable | pass |
| 3 | Prefix matching, single token ("pars"*) | pass | unsupported | unsupported | approximation |
| 3b | Prefix matching, multi-token phrase prefix ("data-api"*) | pass | pass | pass | pass |
| 4 | Fully qualified name search | pass | pass | pass | pass |
| 5 | Function signature search ("sql: &str") | pass | pass | pass | pass |
| 6 | Natural-language documentation search (OR of words) | pass | pass | pass | pass |
| 6b | Documentation search relying on stemming ("running") | pass | expected-semantic | expected-semantic | expected-semantic |
| 7 | Phrase matching ("database query") | pass | pass | pass | pass |
| 8a | Boolean AND | pass | pass | pass | pass |
| 8b | Boolean OR | pass | pass | configurable | pass |
| 8c | Boolean NOT | pass | pass | pass | pass |
| 8d | Boolean OR of phrase prefixes, with NOT | pass | pass | pass | correctness-failure |
| 9a | Identifier tokenization: snake_case part | pass | pass | pass | approximation |
| 9b | Identifier tokenization: camelCase word from search_terms | pass | pass | pass | pass |
| 9c | Identifier tokenization: alphanumeric token (utf8) | pass | pass | pass | pass |
| 9d | Identifier tokenization: single token longer than 40 bytes | pass | configurable | pass | pass |
| 10a | Case-insensitive lookup (PARSE_CONFIG) | pass | pass | configurable | pass |
| 10b | Case-insensitive lookup (httpserver vs HTTPServer) | pass | pass | configurable | pass |
| 11a | Unicode identifier (größe) | pass | pass | pass | pass |
| 11b | Unicode diacritic folding (cafe matches café / Cafés) | pass | expected-semantic | expected-semantic | expected-semantic |
| 11c | Unicode identifier (CJK 数据处理) | pass | pass | pass | pass |
| 11d | Unicode case folding (РАЗОБРАТЬ_СТРОКУ) | pass | pass | configurable | pass |
| 12 | Update indexed content (rename sparse_matrix to dense_grid) | pass | pass | pass | pass |
| 13 | Delete indexed content | pass | pass | pass | pass |
| 14 | Search immediately after commit (second connection) | pass | pass | pass | pass |
| 15 | Search within an uncommitted transaction | pass | pass | pass | pass |
| 16 | Column weighting (name hit outranks docstring hit) | pass | pass | pass | pass |
| 17 | Ranked-result API contract (limit, order, score sign) | pass | pass | pass | pass |
| 18 | Bounded search excluding file/doc kinds | pass | pass | pass | pass |
| 19 | Production search_nodes (phrase channel + OR of prefixes) | pass | unsupported | unsupported | pass |
| 20 | Single-column filter (name : parse) | pass | expected-semantic | expected-semantic | approximation |
| 21a | Memory recall, production SQL shape (newest first, LIMIT 2) | pass | correctness-failure | correctness-failure | correctness-failure |
| 21b | Memory recall, subquery form (newest first, LIMIT 2) | pass | pass | pass | pass |
| 21c | Memory recall with since filter | pass | pass | pass | pass |
| 22a | Executable-body search, production prefix pairs | pass | unsupported | unsupported | pass |
| 22b | Executable-body search, exact-term pairs | pass | pass | pass | pass |
| 23 | Index created before rows, then incremental inserts | pass | pass | pass | pass |
| 24 | Search after closing and reopening the database | pass | pass | pass | pass |
| 25 | FTS call naming a subset of the indexed columns (fts_match(name, 'batching')) | n/a | pass | pass | pass |
| 26 | Stemming tokenizer for a native index (tokenizer='en_stem') | n/a | unsupported | unsupported | unsupported |
