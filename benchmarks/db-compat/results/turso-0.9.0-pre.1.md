| Probe | Gate | libsql-0.9.30 | turso-0.9.0-pre.1 | turso-0.9.0-pre.1-experimental | turso-0.9.0-pre.1-mp-wal |
|---|---|---|---|---|---|
| `fts5.external_content_prefix_or` | FTS5 | pass | error | error | error |
| `fts5.porter_stemming` | FTS5 | pass | error | error | error |
| `fts5.phrase` | FTS5 | pass | error | error | error |
| `fts5.quoted_punctuation_prefix` | FTS5 | pass | error | error | error |
| `fts5.bm25_weighted_order` | FTS5 | pass | error | error | error |
| `fts5.delete_trigger` | FTS5 | pass | error | error | error |
| `fts5.update_trigger` | FTS5 | pass | error | error | error |
| `fts5.rebuild_command` | FTS5 | pass | error | error | error |
| `fts5.unindexed_columns` | FTS5 | pass | error | error | error |
| `cte.recursive_union_all_cycle_guard` | Recursive CTE | pass | pass | pass | pass |
| `cte.recursive_union_dedup` | Recursive CTE | pass | pass | pass | pass |
| `window.named_first_value_row_number` | Window | pass | pass | pass | pass |
| `trigger.when_join_insert` | Triggers | pass | pass | pass | pass |
| `trigger.when_delete` | Triggers | pass | pass | pass | pass |
| `fk.on_delete_cascade` | Foreign keys | pass | pass | pass | pass |
| `fk.enforced_when_on` | Foreign keys | pass | pass | pass | pass |
| `fk.toggle_off_for_bulk_load` | Foreign keys | pass | pass | pass | pass |
| `index.expression_lower` | Expression index | pass | pass | pass | pass |
| `index.unique_expression_rejects_duplicate` | Expression index | pass | pass | pass | pass |
| `conflict.insert_or_ignore_expression_unique` | Conflict clauses | pass | pass | pass | pass |
| `conflict.on_conflict_do_update` | Conflict clauses | pass | pass | pass | pass |
| `conflict.insert_or_replace` | Conflict clauses | pass | pass | pass | pass |
| `table.autoincrement_no_reuse` | Conflict clauses | pass | pass | pass | pass |
| `txn.rollback` | Transactions | pass | pass | pass | pass |
| `txn.begin_exclusive` | Transactions | pass | pass | pass | pass |
| `pragma.page_size` | Pragmas | pass | pass | pass | pass |
| `api.execute_batch_row_returning_pragma` | Rust API | pass | error | error | error |
| `pragma.journal_mode_wal` | Pragmas | pass | pass | pass | pass |
| `pragma.synchronous_normal` | Pragmas | pass | pass | pass | pass |
| `pragma.busy_timeout` | Pragmas | pass | pass | pass | pass |
| `pragma.cache_size` | Pragmas | pass | pass | pass | pass |
| `pragma.temp_store_memory` | Pragmas | pass | pass | pass | pass |
| `pragma.mmap_size` | Pragmas | pass | mismatch | mismatch | mismatch |
| `pragma.user_version` | Pragmas | pass | pass | pass | pass |
| `pragma.table_info` | Pragmas | pass | pass | pass | pass |
| `pragma.query_only_blocks_writes` | Pragmas | pass | pass | pass | pass |
| `pragma.quick_check` | Integrity | pass | pass | pass | pass |
| `pragma.integrity_check` | Integrity | pass | pass | pass | pass |
| `maint.vacuum_analyze` | Maintenance | pass | error | pass | error |
| `func.scalars` | Functions | pass | pass | pass | pass |
| `pragma.wal_checkpoint_truncate` | Pragmas | pass | pass | pass | pass |
| `maint.vacuum_into_snapshot` | Maintenance | pass | pass | pass | pass |
| `persist.reopen` | Persistence | pass | pass | pass | pass |
| `concurrency.second_handle_sees_commit` | Concurrency | pass | pass | pass | pass |
| `file.read_sqlite_written_db` | File compatibility | pass | pass | pass | pass |
| `file.sqlite_reads_tested_written_db` | File compatibility | pass | pass | pass | pass |
| `file.read_sqlite_written_fts_db` | File compatibility | pass | error | error | error |
| `file.sqlite_reads_tested_written_fts_db` | File compatibility | pass | error | error | error |
| `existing.integrity_check` | Existing tokensave DB | pass | error | error | error |
| `existing.user_version` | Existing tokensave DB | pass | error | error | error |
| `existing.count_nodes` | Existing tokensave DB | pass | error | error | error |
| `existing.count_edges` | Existing tokensave DB | pass | error | error | error |
| `existing.fts_search_bm25` | Existing tokensave DB | pass | error | error | error |
| `existing.edges_by_target_kind` | Existing tokensave DB | pass | error | error | error |
| `existing.lower_name_lookup` | Existing tokensave DB | pass | error | error | error |
